use serde::{Deserialize, Deserializer};

use crate::{
    lexer::{LexError, Lexer, Token},
    notification::Notification,
};

const DEFAULT_TEMPLATE: &str = "%s\n%b";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotificationTemplate {
    tokens: Vec<TemplateToken>,
}

impl NotificationTemplate {
    pub fn parse(input: &str) -> Result<Self, TemplateError> {
        let tokens = Lexer::new(input)
            .tokenize()
            .map_err(TemplateError::Lex)?
            .into_iter()
            .map(TemplateToken::try_from)
            .collect::<Result<_, _>>()?;

        Ok(Self { tokens })
    }

    pub fn render(&self, notification: &Notification) -> String {
        let mut output = String::new();

        for run in self.runs(notification) {
            output.push_str(&run.text);
        }

        output
    }

    pub fn is_default_layout(&self) -> bool {
        matches!(
            self.tokens.as_slice(),
            [
                TemplateToken::Field(TemplateField::Summary),
                TemplateToken::Literal(newline),
                TemplateToken::Field(TemplateField::Body),
            ] if newline == "\n"
        )
    }

    pub fn runs(&self, notification: &Notification) -> Vec<TemplateRun> {
        self.tokens
            .iter()
            .map(|token| match token {
                TemplateToken::Literal(text) => TemplateRun {
                    text: text.clone(),
                    element: TemplateElement::Literal,
                },
                TemplateToken::Field(field) => TemplateRun {
                    text: field.resolve(notification),
                    element: field.element(),
                },
            })
            .collect()
    }
}

impl Default for NotificationTemplate {
    fn default() -> Self {
        Self::parse(DEFAULT_TEMPLATE).expect("default notification template is valid")
    }
}

impl<'de> Deserialize<'de> for NotificationTemplate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = String::deserialize(deserializer)?;
        match Self::parse(&input) {
            Ok(template) => Ok(template),
            Err(error) => {
                eprintln!(
                    "invalid notification format {input:?}; using the default layout: {error:?}"
                );
                Ok(Self::default())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TemplateToken {
    Literal(String),
    Field(TemplateField),
}

impl TryFrom<Token> for TemplateToken {
    type Error = TemplateError;

    fn try_from(token: Token) -> Result<Self, Self::Error> {
        match token {
            Token::Literal(text) => Ok(Self::Literal(text)),
            Token::Identifier(identifier) => {
                let field = match identifier.as_str() {
                    "a" => TemplateField::AppName,
                    "s" => TemplateField::Summary,
                    "b" => TemplateField::Body,
                    "c" => TemplateField::Category,
                    "d" => TemplateField::DesktopEntry,
                    "u" => TemplateField::Urgency,
                    "p" => TemplateField::Progress,
                    "S" => TemplateField::StackTag,
                    _ => return Err(TemplateError::UnknownField(identifier)),
                };
                Ok(Self::Field(field))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TemplateField {
    AppName,
    Summary,
    Body,
    Category,
    DesktopEntry,
    Urgency,
    Progress,
    StackTag,
}

impl TemplateField {
    fn resolve(self, notification: &Notification) -> String {
        match self {
            Self::AppName => notification.app_name.clone(),
            Self::Summary => notification.summary.clone(),
            Self::Body => notification.body.clone(),
            Self::Category => notification.category().unwrap_or_default().into(),
            Self::DesktopEntry => notification.desktop_entry().unwrap_or_default().into(),
            Self::Urgency => notification
                .urgency()
                .map_or_else(String::new, |value| value.to_string()),
            Self::Progress => notification
                .progress()
                .map_or_else(String::new, |value| value.to_string()),
            Self::StackTag => notification.stack_tag().unwrap_or_default().into(),
        }
    }

    fn element(self) -> TemplateElement {
        match self {
            Self::AppName => TemplateElement::AppName,
            Self::Summary => TemplateElement::Summary,
            Self::Body => TemplateElement::Body,
            Self::Category
            | Self::DesktopEntry
            | Self::Urgency
            | Self::Progress
            | Self::StackTag => TemplateElement::Details,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateElement {
    Literal,
    AppName,
    Summary,
    Body,
    Details,
}

#[derive(Clone, Debug)]
pub struct TemplateRun {
    pub text: String,
    pub element: TemplateElement,
}

#[derive(Debug)]
pub enum TemplateError {
    Lex(LexError),
    UnknownField(String),
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{NotificationTemplate, TemplateError};
    use crate::{config::NotificationConfig, notification::Notification};

    fn notification() -> Notification {
        Notification {
            id: 1,
            replaces_id: 0,
            app_name: "pigeond-test".into(),
            app_icon: String::new(),
            summary: "Summary".into(),
            body: "Body".into(),
            img: None,
            actions: HashMap::new(),
            hints: HashMap::new(),
            style: NotificationConfig::default(),
        }
    }

    #[test]
    fn renders_fields_and_literals() {
        let template = NotificationTemplate::parse("%a: %s\\n%b").unwrap();

        assert_eq!(
            template.render(&notification()),
            "pigeond-test: Summary\\nBody"
        );
    }

    #[test]
    fn rejects_unknown_fields() {
        assert!(matches!(
            NotificationTemplate::parse("%unknown"),
            Err(TemplateError::UnknownField(field)) if field == "unknown"
        ));
    }

    #[test]
    fn recognizes_the_default_layout() {
        assert!(NotificationTemplate::default().is_default_layout());
    }
}
