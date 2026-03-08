fn main() -> iced::Result {
    iced::application(
        tesuji::gui::GuiApp::new,
        tesuji::gui::GuiApp::update,
        tesuji::gui::GuiApp::view,
    )
    .title("Tesuji")
    .subscription(tesuji::gui::GuiApp::subscription)
    .antialiasing(true)
    .run()
}
