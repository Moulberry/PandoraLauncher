use gpui::{App, Context, IntoElement, Render, Window};

pub trait Page: Sized + Render {
    fn controls(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement;
    fn scrollable(&self, cx: &App) -> bool;
}
