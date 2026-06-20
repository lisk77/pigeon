use crate::images::Image;

#[derive(Clone)]
pub struct Notification {
    pub id: u32,
    pub replaces_id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub img: Option<Image>,
}
