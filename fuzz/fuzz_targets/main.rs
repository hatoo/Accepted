#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    use termion::input::TermRead;
    let syntax_parent = accepted::syntax::SyntaxParent::default();
    let config = accepted::config::ConfigWithDefault::default();

    let mut state: accepted::buffer_tab::BufferTab<accepted::core::buffer::RopeyCoreBuffer> =
        accepted::buffer_tab::BufferTab::new(&syntax_parent, &config);

    for event in data.events() {
        if let Ok(event) = event {
            state.event(event);
        }
    }
});
