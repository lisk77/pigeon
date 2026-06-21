use crate::{
    config::PigeonConfig,
    notification::Notification,
    popup::render::{
        text::{FontCtx, draw_text, measure_text_height},
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
    fonts: &mut FontCtx,
    config: &PigeonConfig,
) {
    let notification_config = &config.notification;
    fill_notification_background(
        canvas,
        width,
        height,
        full_width,
        full_height,
        notification_config.background_color,
        notification_config.border.color,
        notification_config.border.width,
        notification_config.corner_radius,
    );

    let outer_padding = notification_config
        .outer_padding
        .saturating_add(notification_config.border.width);
    let thumbnail_size = notification_config.thumbnail.size;
    let thumbnail_gap = notification_config.thumbnail.gap;
    let summary_font_size = notification_config.summary.font_size;
    let body_font_size = notification_config.body.font_size;
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
    let summary_height = measure_text_height(
        fonts,
        &notification.summary,
        text_width,
        summary_font_size,
        true,
    );
    let body_y = outer_padding as f32 + summary_height + summary_body_gap;

    let body_y = body_y.ceil() as u32;
    let body_height = full_height
        .saturating_sub(body_y)
        .saturating_sub(outer_padding);

    draw_text(
        canvas,
        width,
        height,
        fonts,
        &notification.summary,
        text_x,
        outer_padding,
        text_width,
        summary_height.ceil() as u32,
        summary_font_size,
        true,
    );

    draw_text(
        canvas,
        width,
        height,
        fonts,
        &notification.body,
        text_x,
        body_y,
        text_width,
        body_height,
        body_font_size,
        false,
    );
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
                _ if border_width == 0 => background,
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
                    background
                }
                _ => border,
            };
            canvas[pixel..pixel + 4].copy_from_slice(&color);
        }
    }
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
    width: u32,
    fonts: &mut FontCtx,
    config: &PigeonConfig,
) -> u32 {
    let notification_config = &config.notification;
    let content_inset = notification_config
        .outer_padding
        .saturating_add(notification_config.border.width);
    let text_x = if notification.img.is_some() {
        content_inset + notification_config.thumbnail.size + notification_config.thumbnail.gap
    } else {
        content_inset
    };
    let text_width = width.saturating_sub(text_x).saturating_sub(content_inset);
    let summary_height = measure_text_height(
        fonts,
        &notification.summary,
        text_width,
        notification_config.summary.font_size,
        true,
    );
    let body_height = measure_text_height(
        fonts,
        &notification.body,
        text_width,
        notification_config.body.font_size,
        false,
    );
    let text_stack_height = summary_height + notification_config.summary.bottom_gap + body_height;
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
