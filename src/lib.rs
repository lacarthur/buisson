use lessons::NodeStatus;
use ratatui::style::{Style, Stylize};

pub mod app;
pub mod components;
pub mod lessons;

pub fn style_from_status(status: &NodeStatus) -> Style {
    match status {
        NodeStatus::Ok => Style::default().light_green(),
        NodeStatus::Pending => Style::default().light_yellow(),
        NodeStatus::MissingPrereq(_) => Style::default().light_red(),
    }
}
