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
