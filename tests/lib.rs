use termion::event::{Event, Key};

use accepted::core::Cursor;
use accepted::{config, core::buffer::RopeyCoreBuffer, core::CoreBuffer, Buffer, BufferMode};

trait BufferModeExt {
    fn command(&mut self, command: &str);
    fn command_esc(&mut self, command: &str);
}

impl<'a, B: CoreBuffer> BufferModeExt for BufferMode<'a, B> {
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
fn with_buffer_mode<F: FnOnce(BufferMode<RopeyCoreBuffer>)>(func: F) {
    let syntax_parent = accepted::syntax::SyntaxParent::default();
    let config = config::ConfigWithDefault::default();
    let buf = Buffer::new(&syntax_parent, &config);
    let state = BufferMode::new(buf);
    func(state)
}

fn with_buffer_mode_from<T, F: FnOnce(BufferMode<RopeyCoreBuffer>) -> T>(init: &str, func: F) -> T {
    let syntax_parent = accepted::syntax::SyntaxParent::default();
    let config = config::ConfigWithDefault::default();
    let mut buf = Buffer::new(&syntax_parent, &config);
    buf.core.set_string(init.into(), true);
    let state = BufferMode::new(buf);
    func(state)
}

fn simple_run(init: &str, commands: &str) -> String {
    with_buffer_mode_from(init, |mut state| {
        state.command_esc(commands);
        state.buf.core.get_string()
    })
}

fn test_run(init: &str, commands: &[&str]) -> (String, Cursor) {
    with_buffer_mode_from(init, |mut state| {
        for command in commands {
            state.command_esc(command);
        }
        (state.buf.core.get_string(), state.buf.core.cursor())
    })
}

#[test]
fn test_simples() {
    // Insertions
    assert_eq!(simple_run("123", "iHello World"), "Hello World123");
    assert_eq!(simple_run("123", "iHello\nWorld"), "Hello\nWorld123");
    assert_eq!(simple_run("123", "llllllIHello World"), "Hello World123");
    assert_eq!(
        simple_run("    123", "lllllllllllllllllllIHello World"),
        "    Hello World123"
    );
    assert_eq!(simple_run("123", "aHello World"), "1Hello World23");
    assert_eq!(simple_run("123", "AHello World"), "123Hello World");
    assert_eq!(simple_run("123", "oHello World"), "123\nHello World");
    assert_eq!(simple_run("123", "OHello World"), "Hello World\n123");

    // r
    assert_eq!(simple_run("123", "r8"), "823");

    // s, S, C
    assert_eq!(simple_run("123", "sHello World"), "Hello World23");
    assert_eq!(simple_run("123", "SHello World"), "Hello World");
    assert_eq!(simple_run("123 456 789", "wCabc"), "123 abc");

    // dd
    assert_eq!(simple_run("1\n2\n3", "dd"), "2\n3");
    assert_eq!(simple_run("1\n2\n3", "ddp"), "2\n1\n3");
    assert_eq!(simple_run("1\n2\n3", "jdd"), "1\n3");
    // cc
    assert_eq!(simple_run("1\n2\n3", "cca"), "a\n2\n3");
    // dj
    assert_eq!(simple_run("1\n2\n3\n4", "jdj"), "1\n4");
    assert_eq!(simple_run("1\n2\n3\n4", "jdjp"), "1\n4\n2\n3");
    // cj
    assert_eq!(simple_run("1\n222\n333\n4", "jcja"), "1\na\n4");
    // dk
    assert_eq!(simple_run("1\n2\n3\n4", "jjdk"), "1\n4");
    assert_eq!(simple_run("1\n2\n3\n4", "jjdkp"), "1\n4\n2\n3");

    // word
    assert_eq!(simple_run("123 456 789", "wdw"), "123 789");
    assert_eq!(simple_run("123 456 789", "dw"), "456 789");
    assert_eq!(simple_run("123 456             789", "wdw"), "123 789");
    assert_eq!(simple_run("123 456 789", "wdaw"), "123 789");
    assert_eq!(simple_run("123 456 789", "daw"), "456 789");
    assert_eq!(simple_run("123 456 789", "ldaw"), "456 789");
    assert_eq!(simple_run("123 456             789", "wdaw"), "123 789");
    assert_eq!(simple_run("123 456 789", "wdiw"), "123  789");
    assert_eq!(simple_run("123 456 789", "diw"), " 456 789");
    assert_eq!(simple_run("123 456 789", "ldiw"), " 456 789");

    assert_eq!(simple_run("123 456 789", "wcw"), "123  789");
    assert_eq!(simple_run("123 456 789", "wcwabc"), "123 abc 789");
    assert_eq!(simple_run("123 456   789", "wcw"), "123    789");
    assert_eq!(simple_run("123 456 789", "wcaw"), "123  789");
    assert_eq!(simple_run("123 456   789", "wcaw"), "123    789");
    assert_eq!(simple_run("123 456 789", "wciw"), "123  789");
    assert_eq!(simple_run("123 456   789", "wciw"), "123    789");

    // parens
    assert_eq!(simple_run("(123 456 789)(abc)", "di)"), "()(abc)");
    assert_eq!(simple_run("(123 456 789)(abc)", "da)"), "(abc)");
    assert_eq!(simple_run("((123 456 789)(qwe))(abc)", "da)"), "(abc)");

    // Quote
    assert_eq!(simple_run("\"123 456 789\"\"abc\"", "di\""), "\"\"\"abc\"");
    assert_eq!(simple_run("\"123 456 789\"\"abc\"", "da\""), "\"abc\"");

    // f,t
    assert_eq!(simple_run("123456", "df4"), "56");
    assert_eq!(simple_run("123456", "dt4"), "456");
    assert_eq!(simple_run("123456abc", "dta"), "abc");

    // Yanks
    assert_eq!(simple_run("123\n456\n789", "yyp"), "123\n123\n456\n789");
    assert_eq!(simple_run("123\n456\n789", "ddp"), "456\n123\n789");
    assert_eq!(simple_run("123 456 789", "dwwP"), "456 123 789");

    // 0, $
    assert_eq!(simple_run("123 456 789", "ww0iabc "), "abc 123 456 789");
    assert_eq!(simple_run("123 456 789", "$i abc"), "123 456 789 abc");

    // Auto indent
    assert_eq!(simple_run("123", "A\n"), "123\n");
    assert_eq!(simple_run("123{", "A\n"), "123{\n    ");

    // g
    assert_eq!(simple_run("123\n456", "wwwwwgiabc"), "abc123\n456");
    assert_eq!(simple_run("123\n456", "Giabc"), "123\n456abc");

    // f, F
    assert_eq!(simple_run("123456", "f4ia"), "123a456");
    assert_eq!(simple_run("123456", "$F4ia"), "123a456");

    // search
    assert_eq!(simple_run("123\nabc\n456", "/abc\nniz"), "123\nzabc\n456");
    assert_eq!(simple_run("123\nabc\n456", "G/abc\nNiz"), "123\nzabc\n456");

    // Run
    // Not crash
    assert_eq!(
        simple_run("#!/bin/sh\n\necho Hello", " tgG"),
        "#!/bin/sh\n\necho Hello"
    );

    // Visual
    assert_eq!(simple_run("123 456 789", "ved"), " 456 789");
    assert_eq!(simple_run("123 456 789", "vesabc"), "abc 456 789");
    assert_eq!(simple_run("123\n456\n789", "Vd"), "456\n789");
    assert_eq!(simple_run("123\n456\n789", "Vjd"), "789");
    assert_eq!(simple_run("123\n456\n789", "Vyp"), "123\n123\n456\n789");

    // Goto
    assert_eq!(simple_run("123\n456\n789", " g2\nix"), "123\nx456\n789");
}
