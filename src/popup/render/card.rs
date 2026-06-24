use crate::{
    config::notification::{
        NotificationConfig, ProgressAlignment, ProgressConfig, ProgressDirection, TemplateElement,
    },
    notification::Notification,
    popup::render::{
        text::{FontCtx, StyledTextRun, draw_styled_text, measure_styled_text_height},
        thumbnail::draw_thumbnail,
    },
};

pub fn render_card(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    full_width: u32,
    full_height: u32,
    notification: &Notification,
    notification_config: &NotificationConfig,
    fonts: &mut FontCtx,
) {
    let progress = progress_rect(
        notification,
        full_width,
        full_height,
        notification_config.border.width,
        &notification_config.progress,
    );
    fill_notification_background(
        canvas,
        width,
        height,
        full_width,
        full_height,
        notification_config.color,
        notification_config.border.color,
        notification_config.border.width,
        notification_config.corner_radius,
        progress,
        notification_config.progress.color,
    );

    let outer_padding = notification_config
        .outer_padding
        .saturating_add(notification_config.border.width);
    let thumbnail_size = notification_config.thumbnail.size;
    let thumbnail_gap = notification_config.thumbnail.gap;
    let summary_body_gap = notification_config.summary.bottom_gap;

    let text_x = if let Some(img) = &notification.img {
        draw_thumbnail(
            canvas,
            width,
            height,
            img,
            outer_padding,
            outer_padding,
            thumbnail_size,
            thumbnail_size,
        );
        outer_padding + thumbnail_size + thumbnail_gap
    } else {
        outer_padding
    };
    let text_width = full_width
        .saturating_sub(text_x)
        .saturating_sub(outer_padding);
    if notification_config.format.is_default_layout() {
        let summary_runs = [StyledTextRun {
            text: &notification.summary,
            style: &notification_config.summary.text,
        }];
        let body_runs = [StyledTextRun {
            text: &notification.body,
            style: &notification_config.body.text,
        }];
        let summary_height = measure_styled_text_height(
            fonts,
            &summary_runs,
            text_width,
            &notification_config.summary.text,
            &notification_config.emoji_font,
        );
        let body_y = (outer_padding as f32 + summary_height + summary_body_gap).ceil() as u32;
        let body_height = full_height
            .saturating_sub(body_y)
            .saturating_sub(outer_padding);

        draw_styled_text(
            canvas,
            width,
            height,
            fonts,
            &summary_runs,
            text_x,
            outer_padding,
            text_width,
            summary_height.ceil() as u32,
            &notification_config.summary.text,
            &notification_config.emoji_font,
        );

        draw_styled_text(
            canvas,
            width,
            height,
            fonts,
            &body_runs,
            text_x,
            body_y,
            text_width,
            body_height,
            &notification_config.body.text,
            &notification_config.emoji_font,
        );
    } else {
        let runs = notification_config.format.runs(notification);
        let styled_runs = styled_template_runs(&runs, notification_config);
        let text_height = full_height.saturating_sub(outer_padding.saturating_mul(2));
        draw_styled_text(
            canvas,
            width,
            height,
            fonts,
            &styled_runs,
            text_x,
            outer_padding,
            text_width,
            text_height,
            &notification_config.body.text,
            &notification_config.emoji_font,
        );
    }
}

fn fill_notification_background(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    full_width: u32,
    full_height: u32,
    background: [u8; 4],
    border: [u8; 4],
    border_width: u32,
    corner_radius: u32,
    progress: Option<ProgressRect>,
    progress_color: [u8; 4],
) {
    let border_width = border_width.min(full_width / 2).min(full_height / 2);
    let inner_width = full_width.saturating_sub(border_width.saturating_mul(2));
    let inner_height = full_height.saturating_sub(border_width.saturating_mul(2));
    let inner_radius = corner_radius.saturating_sub(border_width);

    for y in 0..height {
        for x in 0..width {
            let pixel = ((y * width + x) * 4) as usize;
            let color = match () {
                _ if !rounded_rect_contains(x, y, full_width, full_height, corner_radius) => {
                    [0, 0, 0, 0]
                }
                _ if border_width == 0 => {
                    apply_progress(background, x, y, progress, progress_color)
                }
                _ if inner_width > 0
                    && inner_height > 0
                    && x >= border_width
                    && y >= border_width
                    && rounded_rect_contains(
                        x - border_width,
                        y - border_width,
                        inner_width,
                        inner_height,
                        inner_radius,
                    ) =>
                {
                    apply_progress(background, x, y, progress, progress_color)
                }
                _ => border,
            };
            canvas[pixel..pixel + 4].copy_from_slice(&color);
        }
    }
}

#[derive(Clone, Copy)]
struct ProgressRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    corner_radius: u32,
}

impl ProgressRect {
    fn contains(self, x: u32, y: u32) -> bool {
        x >= self.x
            && y >= self.y
            && rounded_rect_contains(
                x - self.x,
                y - self.y,
                self.width,
                self.height,
                self.corner_radius,
            )
    }
}

fn progress_rect(
    notification: &Notification,
    full_width: u32,
    full_height: u32,
    border_width: u32,
    config: &ProgressConfig,
) -> Option<ProgressRect> {
    let value = notification.progress()?.clamp(0, 100) as u32;
    if value == 0 {
        return None;
    }

    let border_width = border_width.min(full_width / 2).min(full_height / 2);
    let inner_width = full_width.saturating_sub(border_width.saturating_mul(2));
    let inner_height = full_height.saturating_sub(border_width.saturating_mul(2));
    let inset = config.inset.min(inner_width / 2).min(inner_height / 2);
    let x = border_width.saturating_add(inset);
    let y = border_width.saturating_add(inset);
    let width = inner_width.saturating_sub(inset.saturating_mul(2));
    let height = inner_height.saturating_sub(inset.saturating_mul(2));

    if config.direction.is_horizontal() {
        let thickness = config.thickness.resolve(height);
        let fill_width = width.saturating_mul(value) / 100;
        if thickness == 0 || fill_width == 0 {
            return None;
        }

        let y = y.saturating_add(aligned_offset(height, thickness, &config.alignment));
        let x = match config.direction {
            ProgressDirection::RightToLeft => x.saturating_add(width.saturating_sub(fill_width)),
            _ => x,
        };
        Some(ProgressRect {
            x,
            y,
            width: fill_width,
            height: thickness,
            corner_radius: config.corner_radius,
        })
    } else {
        let thickness = config.thickness.resolve(width);
        let fill_height = height.saturating_mul(value) / 100;
        if thickness == 0 || fill_height == 0 {
            return None;
        }

        let x = x.saturating_add(aligned_offset(width, thickness, &config.alignment));
        let y = match config.direction {
            ProgressDirection::BottomToTop => y.saturating_add(height.saturating_sub(fill_height)),
            _ => y,
        };
        Some(ProgressRect {
            x,
            y,
            width: thickness,
            height: fill_height,
            corner_radius: config.corner_radius,
        })
    }
}

fn aligned_offset(available: u32, thickness: u32, alignment: &ProgressAlignment) -> u32 {
    match alignment {
        ProgressAlignment::Start => 0,
        ProgressAlignment::Center => available.saturating_sub(thickness) / 2,
        ProgressAlignment::End => available.saturating_sub(thickness),
    }
}

fn apply_progress(
    background: [u8; 4],
    x: u32,
    y: u32,
    progress: Option<ProgressRect>,
    progress_color: [u8; 4],
) -> [u8; 4] {
    if progress.is_some_and(|progress| progress.contains(x, y)) {
        blend_color(background, progress_color)
    } else {
        background
    }
}

fn blend_color(background: [u8; 4], foreground: [u8; 4]) -> [u8; 4] {
    let alpha = u16::from(foreground[3]);
    let inverse_alpha = 255 - alpha;

    [
        ((u16::from(foreground[0]) * alpha + u16::from(background[0]) * inverse_alpha) / 255) as u8,
        ((u16::from(foreground[1]) * alpha + u16::from(background[1]) * inverse_alpha) / 255) as u8,
        ((u16::from(foreground[2]) * alpha + u16::from(background[2]) * inverse_alpha) / 255) as u8,
        background[3].max(foreground[3]),
    ]
}

fn rounded_rect_contains(x: u32, y: u32, width: u32, height: u32, corner_radius: u32) -> bool {
    if x >= width || y >= height {
        return false;
    }

    let radius = corner_radius.min(width / 2).min(height / 2);
    if radius == 0 || (x >= radius && x < width - radius) || (y >= radius && y < height - radius) {
        return true;
    }

    let center_x = if x < radius {
        radius as u64 * 2
    } else {
        (width - radius) as u64 * 2
    };
    let center_y = if y < radius {
        radius as u64 * 2
    } else {
        (height - radius) as u64 * 2
    };
    let pixel_x = x as u64 * 2 + 1;
    let pixel_y = y as u64 * 2 + 1;
    let radius = radius as u64 * 2;

    center_x.abs_diff(pixel_x).pow(2) + center_y.abs_diff(pixel_y).pow(2) <= radius.pow(2)
}

pub fn measure_card_height(
    notification: &Notification,
    notification_config: &NotificationConfig,
    width: u32,
    fonts: &mut FontCtx,
) -> u32 {
    let content_inset = notification_config
        .outer_padding
        .saturating_add(notification_config.border.width);
    let text_x = if notification.img.is_some() {
        content_inset + notification_config.thumbnail.size + notification_config.thumbnail.gap
    } else {
        content_inset
    };
    let text_width = width.saturating_sub(text_x).saturating_sub(content_inset);
    let text_stack_height = if notification_config.format.is_default_layout() {
        let summary_runs = [StyledTextRun {
            text: &notification.summary,
            style: &notification_config.summary.text,
        }];
        let body_runs = [StyledTextRun {
            text: &notification.body,
            style: &notification_config.body.text,
        }];
        let summary_height = measure_styled_text_height(
            fonts,
            &summary_runs,
            text_width,
            &notification_config.summary.text,
            &notification_config.emoji_font,
        );
        let body_height = measure_styled_text_height(
            fonts,
            &body_runs,
            text_width,
            &notification_config.body.text,
            &notification_config.emoji_font,
        );
        summary_height + notification_config.summary.bottom_gap + body_height
    } else {
        let runs = notification_config.format.runs(notification);
        let styled_runs = styled_template_runs(&runs, notification_config);
        measure_styled_text_height(
            fonts,
            &styled_runs,
            text_width,
            &notification_config.body.text,
            &notification_config.emoji_font,
        )
    };
    let content_height = if notification.img.is_some() {
        (notification_config.thumbnail.size as f32).max(text_stack_height)
    } else {
        text_stack_height
    };

    ((content_inset.saturating_mul(2) as f32 + content_height).ceil() as u32).clamp(
        notification_config.min_height,
        notification_config.max_height,
    )
}

fn styled_template_runs<'a>(
    runs: &'a [crate::config::notification::TemplateRun],
    config: &'a NotificationConfig,
) -> Vec<StyledTextRun<'a>> {
    runs.iter()
        .map(|run| StyledTextRun {
            text: &run.text,
            style: match run.element {
                TemplateElement::Literal => &config.literal,
                TemplateElement::AppName => &config.app_name,
                TemplateElement::Summary => &config.summary.text,
                TemplateElement::Body => &config.body.text,
                TemplateElement::Details => &config.details,
            },
        })
        .collect()
}
