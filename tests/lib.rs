use termion::event::{Event, Key};
use accepted::{BufferMode,  Buffer};

trait BufferModeExt {
    fn command(&mut self, command: &str);
    fn command_esc(&mut self, command: &str);
}

impl<'a> BufferModeExt for BufferMode<'a> {
    fn command(&mut self, command: &str) {
        for c in command.chars() {
            self.event(Event::Key(Key::Char(c)));
        }
    }
    fn command_esc(&mut self, command: &str) {
        self.command(command);
        self.event(Event::Key(Key::Esc));
    }
}

fn with_buffer_mode<F: FnOnce(BufferMode)>(func: F) {
    let syntax_parent = accepted::syntax::SyntaxParent::default();
    let buf = Buffer::new(&syntax_parent);
    let state = BufferMode::new(buf);
    func(state)
}

#[test]
fn test_hello_world() {
    with_buffer_mode(|mut state| {
        state.command_esc("iHello World");
        assert_eq!(state.buf.core.get_string(), "Hello World");
    });
}