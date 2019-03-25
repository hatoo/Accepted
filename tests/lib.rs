use termion::event::{Event, Key};

use accepted::{config, Buffer, BufferMode};

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

#[allow(dead_code)]
fn with_buffer_mode<F: FnOnce(BufferMode)>(func: F) {
    let syntax_parent = accepted::syntax::SyntaxParent::default();
    let config = config::ConfigWithDefault::default();
    let buf = Buffer::new(&syntax_parent, &config);
    let state = BufferMode::new(buf);
    func(state)
}

fn with_buffer_mode_from<F: FnOnce(BufferMode)>(init: &str, func: F) {
    let syntax_parent = accepted::syntax::SyntaxParent::default();
    let config = config::ConfigWithDefault::default();
    let mut buf = Buffer::new(&syntax_parent, &config);
    buf.core.set_string(init.into(), true);
    let state = BufferMode::new(buf);
    func(state)
}

fn simple_test(init: &str, commands: &str, expected: &str) {
    with_buffer_mode_from(init, |mut state| {
        state.command_esc(commands);
        assert_eq!(state.buf.core.get_string(), expected);
    });
}

#[test]
fn test_simples() {
    // Insertions
    simple_test("123", "iHello World", "Hello World123");
    simple_test("123", "llllllIHello World", "Hello World123");
    simple_test("123", "aHello World", "1Hello World23");
    simple_test("123", "AHello World", "123Hello World");
    simple_test("123", "oHello World", "123\nHello World");
    simple_test("123", "OHello World", "Hello World\n123");

    // r
    simple_test("123", "r8", "823");

    // s, S, C
    simple_test("123", "sHello World", "Hello World23");
    simple_test("123", "SHello World", "Hello World");
    simple_test("123 456 789", "wCabc", "123 abc");

    // dd
    simple_test("1\n2\n3", "jdd", "1\n3");
    // dj
    simple_test("1\n2\n3\n4", "jdj", "1\n4");
    // dk
    simple_test("1\n2\n3\n4", "jjdk", "1\n4");

    // word
    simple_test("123 456 789", "wdw", "123 789");
    simple_test("123 456 789", "dw", "456 789");
    simple_test("123 456             789", "wdw", "123 789");
    simple_test("123 456 789", "wdaw", "123 789");
    simple_test("123 456 789", "daw", "456 789");
    simple_test("123 456 789", "ldaw", "456 789");
    simple_test("123 456             789", "wdaw", "123 789");
    simple_test("123 456 789", "wdiw", "123  789");
    simple_test("123 456 789", "diw", " 456 789");
    simple_test("123 456 789", "ldiw", " 456 789");

    simple_test("123 456 789", "wcw", "123  789");
    simple_test("123 456 789", "wcwabc", "123 abc 789");
    simple_test("123 456   789", "wcw", "123    789");
    simple_test("123 456 789", "wcaw", "123  789");
    simple_test("123 456   789", "wcaw", "123    789");
    simple_test("123 456 789", "wciw", "123  789");
    simple_test("123 456   789", "wciw", "123    789");
    // parens
    simple_test("(123 456 789)(abc)", "di)", "()(abc)");
    simple_test("(123 456 789)(abc)", "da)", "(abc)");
    // f,t
    simple_test("123456", "df4", "56");
    simple_test("123456", "dt4", "456");
    simple_test("123456abc", "dta", "abc");

    // Yanks
    simple_test("123\n456\n789", "yyp", "123\n123\n456\n789");
    simple_test("123\n456\n789", "ddp", "456\n123\n789");
    simple_test("123 456 789", "dwwP", "456 123 789");

    // 0, $
    simple_test("123 456 789", "ww0iabc ", "abc 123 456 789");
    simple_test("123 456 789", "$i abc", "123 456 789 abc");

    // Auto indent
    simple_test("123", "A\n", "123\n");
    simple_test("123{", "A\n", "123{\n    ");
}
