fn main() -> iced::Result {
    use tesuji_gui::gui::GuiApp;
    iced::application(GuiApp::new, GuiApp::update, GuiApp::view)
        .title("Tesuji")
        .subscription(GuiApp::subscription)
        .antialiasing(true)
        .run()
}
