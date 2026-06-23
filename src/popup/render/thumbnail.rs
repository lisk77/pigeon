use crate::images::Image;

pub fn draw_thumbnail(
    canvas: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    image: &Image,
    box_x: u32,
    box_y: u32,
    box_width: u32,
    box_height: u32,
) {
    let thumbnail = image.inner();

    let draw_x = box_x + (box_width - thumbnail.width()) / 2;
    let draw_y = box_y + (box_height - thumbnail.height()) / 2;

    for (x, y, pixel) in thumbnail.enumerate_pixels() {
        let target_x = draw_x + x;
        let target_y = draw_y + y;

        if target_x >= canvas_width || target_y >= canvas_height {
            continue;
        }

        let [red, green, blue, alpha] = pixel.0;
        let index = ((target_y * canvas_width + target_x) * 4) as usize;

        let alpha = u16::from(alpha);
        let inverse_alpha = 255 - alpha;

        canvas[index] =
            ((u16::from(blue) * alpha + u16::from(canvas[index]) * inverse_alpha) / 255) as u8;

        canvas[index + 1] =
            ((u16::from(green) * alpha + u16::from(canvas[index + 1]) * inverse_alpha) / 255) as u8;

        canvas[index + 2] =
            ((u16::from(red) * alpha + u16::from(canvas[index + 2]) * inverse_alpha) / 255) as u8;

        canvas[index + 3] = 0xFF;
    }
}
