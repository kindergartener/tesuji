fn main() -> iced::Result {
    iced::application(
        tesuji::gui::GuiApp::new,
        tesuji::gui::GuiApp::update,
        tesuji::gui::GuiApp::view,
    )
    .title("Tesuji")
    .run()
}
