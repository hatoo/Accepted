use termion::event::{Event, Key};

use accepted::core::Cursor;
use accepted::{config, core::buffer::RopeyCoreBuffer, core::CoreBuffer, Buffer, BufferMode};
use async_trait::async_trait;

#[async_trait(?Send)]
trait BufferModeExt {
    async fn command(&mut self, command: &str);
    async fn command_esc(&mut self, command: &str);
}

#[async_trait(?Send)]
impl<'a, B: CoreBuffer> BufferModeExt for BufferMode<'a, B> {
    async fn command(&mut self, command: &str) {
        for c in command.chars() {
            self.event(Event::Key(Key::Char(c))).await;
        }
    }
    async fn command_esc(&mut self, command: &str) {
        self.command(command).await;
        self.event(Event::Key(Key::Esc)).await;
    }
}

/*
fn buffer_mode_from(init: &str) -> BufferMode<RopeyCoreBuffer> {
    let syntax_parent = accepted::syntax::SyntaxParent::default();
    let config = config::ConfigWithDefault::default();
    let mut buf = Buffer::new(&syntax_parent, &config);
    buf.core.set_string(init.into(), true);
    BufferMode::new(buf)
}

fn buffer_mode_from_config(
    config: &config::ConfigWithDefault,
    init: &str,
) -> BufferMode<RopeyCoreBuffer> {
    let syntax_parent = accepted::syntax::SyntaxParent::default();
    let mut buf = Buffer::new(&syntax_parent, config);
    buf.core.set_string(init.into(), true);
    BufferMode::new(buf)
}
*/

async fn simple_run(init: &str, commands: &str) -> String {
    let syntax_parent = accepted::syntax::SyntaxParent::default();
    let config = config::ConfigWithDefault::default();
    let mut buf: Buffer<RopeyCoreBuffer> = Buffer::new(&syntax_parent, &config);
    buf.core.set_string(init.into(), true);
    let mut buffer = BufferMode::new(buf);
    buffer.command_esc(commands).await;
    buffer.buf.core.get_string()
}

async fn simple_run_config(
    config: &config::ConfigWithDefault,
    init: &str,
    commands: &str,
) -> String {
    let syntax_parent = accepted::syntax::SyntaxParent::default();
    let mut buf: Buffer<RopeyCoreBuffer> = Buffer::new(&syntax_parent, &config);
    buf.core.set_string(init.into(), true);
    let mut buffer = BufferMode::new(buf);
    buffer.command_esc(commands).await;
    buffer.buf.core.get_string()
}

async fn test_from_fuzz(data: &[u8]) {
    use accepted::buffer_tab::BufferTab;
    use termion::input::TermRead;

    let syntax_parent = accepted::syntax::SyntaxParent::default();
    let config = config::ConfigWithDefault::default();
    let mut buf = BufferTab::<RopeyCoreBuffer>::new(&syntax_parent, &config);

    for ev in data.events() {
        if let Ok(ev) = ev {
            buf.event(ev).await;
        }
    }
}

#[tokio::test]
async fn test_simples() {
    // Insertions
    assert_eq!(simple_run("123", "iHello World").await, "Hello World123");
    assert_eq!(simple_run("123", "iHello\nWorld").await, "Hello\nWorld123");
    assert_eq!(
        simple_run("123", "llllllIHello World").await,
        "Hello World123"
    );
    assert_eq!(
        simple_run("    123", "lllllllllllllllllllIHello World").await,
        "    Hello World123"
    );
    assert_eq!(simple_run("123", "aHello World").await, "1Hello World23");
    assert_eq!(simple_run("123", "AHello World").await, "123Hello World");
    assert_eq!(simple_run("123", "oHello World").await, "123\nHello World");
    assert_eq!(simple_run("123", "OHello World").await, "Hello World\n123");

    // x
    assert_eq!(simple_run("", "x").await, "");

    // r
    assert_eq!(simple_run("", "ra").await, "a");
    assert_eq!(simple_run("123", "r8").await, "823");

    // s, S, C
    assert_eq!(simple_run("123", "sHello World").await, "Hello World23");
    assert_eq!(simple_run("123", "SHello World").await, "Hello World");
    assert_eq!(simple_run("123 456 789", "wCabc").await, "123 abc");

    // dd
    assert_eq!(simple_run("", "dd").await, "");
    assert_eq!(simple_run("1", "ddP").await, "1\n");
    assert_eq!(simple_run("1\n2\n3", "dd").await, "2\n3");
    assert_eq!(simple_run("1\n2\n3", "ddp").await, "2\n1\n3");
    assert_eq!(simple_run("1\n2\n3", "jdd").await, "1\n3");
    assert_eq!(simple_run("1\n2", "jdd").await, "1");
    // cc
    assert_eq!(simple_run("1", "cca").await, "a");
    assert_eq!(simple_run("1\n2\n3", "cca").await, "a\n2\n3");
    // dj
    assert_eq!(simple_run("1\n2", "djP").await, "1\n2\n");
    assert_eq!(simple_run("1\n2\n3\n4", "jdj").await, "1\n4");
    assert_eq!(simple_run("1\n2\n3\n4", "jdjp").await, "1\n4\n2\n3");
    // cj
    assert_eq!(simple_run("1\n222\n333\n4", "jcja").await, "1\na\n4");
    // dk
    assert_eq!(simple_run("1\n2\n3\n4", "jjdk").await, "1\n4");
    assert_eq!(simple_run("1\n2\n3\n4", "jjdkp").await, "1\n4\n2\n3");

    // word
    assert_eq!(simple_run("123 456 789", "wdw").await, "123 789");
    assert_eq!(simple_run("123 456 789", "dw").await, "456 789");
    assert_eq!(
        simple_run("123 456             789", "wdw").await,
        "123 789"
    );
    assert_eq!(simple_run("123 456 789", "wdaw").await, "123 789");
    assert_eq!(simple_run("123 456 789", "daw").await, "456 789");
    assert_eq!(simple_run("123 456 789", "ldaw").await, "456 789");
    assert_eq!(
        simple_run("123 456             789", "wdaw").await,
        "123 789"
    );
    assert_eq!(simple_run("123 456 789", "wdiw").await, "123  789");
    assert_eq!(simple_run("123 456 789", "diw").await, " 456 789");
    assert_eq!(simple_run("123 456 789", "ldiw").await, " 456 789");

    assert_eq!(simple_run("123 456 789", "wcw").await, "123  789");
    assert_eq!(simple_run("123 456 789", "wcwabc").await, "123 abc 789");
    assert_eq!(simple_run("123 456   789", "wcw").await, "123    789");
    assert_eq!(simple_run("123 456 789", "wcaw").await, "123  789");
    assert_eq!(simple_run("123 456   789", "wcaw").await, "123    789");
    assert_eq!(simple_run("123 456 789", "wciw").await, "123  789");
    assert_eq!(simple_run("123 456   789", "wciw").await, "123    789");

    // parens
    assert_eq!(simple_run("(123 456 789)(abc)", "di)").await, "()(abc)");
    assert_eq!(simple_run("(123 456 789)(abc)", "da)").await, "(abc)");
    assert_eq!(
        simple_run("((123 456 789)(qwe))(abc)", "da)").await,
        "(abc)"
    );

    // Quote
    assert_eq!(
        simple_run("\"123 456 789\"\"abc\"", "di\"").await,
        "\"\"\"abc\""
    );
    assert_eq!(
        simple_run("\"123 456 789\"\"abc\"", "da\"").await,
        "\"abc\""
    );

    // f,t
    assert_eq!(simple_run("123456", "df4").await, "56");
    assert_eq!(simple_run("123456", "dt4").await, "456");
    assert_eq!(simple_run("123456abc", "dta").await, "abc");

    // Yanks
    assert_eq!(
        simple_run("123\n456\n789", "yyp").await,
        "123\n123\n456\n789"
    );
    assert_eq!(simple_run("123\n456\n789", "ddp").await, "456\n123\n789");
    assert_eq!(simple_run("123 456 789", "dwwP").await, "456 123 789");

    // 0, $
    assert_eq!(
        simple_run("123 456 789", "ww0iabc ").await,
        "abc 123 456 789"
    );
    assert_eq!(simple_run("123 456 789", "$i abc").await, "123 456 789 abc");

    // Auto indent
    assert_eq!(simple_run("123", "A\n").await, "123\n");
    assert_eq!(simple_run("123{", "A\n").await, "123{\n    ");

    // g
    assert_eq!(simple_run("123\n456", "wwwwwgiabc").await, "abc123\n456");
    assert_eq!(simple_run("123\n456", "Giabc").await, "123\n456abc");

    // f, F
    assert_eq!(simple_run("123456", "f4ia").await, "123a456");
    assert_eq!(simple_run("123456", "$F4ia").await, "123a456");

    // search
    assert_eq!(
        simple_run("123\nabc\n456", "/abc\nniz").await,
        "123\nzabc\n456"
    );
    assert_eq!(
        simple_run("123\nabc\n456", "G/abc\nNiz").await,
        "123\nzabc\n456"
    );

    // Run
    // Not crash
    assert_eq!(
        simple_run("#!/bin/sh\n\necho Hello", " tgG").await,
        "#!/bin/sh\n\necho Hello"
    );

    // Visual
    assert_eq!(simple_run("", "vx").await, "");
    assert_eq!(simple_run("", "Vx").await, "");
    assert_eq!(simple_run("123 456 789", "ved").await, " 456 789");
    assert_eq!(simple_run("123 456 789", "vesabc").await, "abc 456 789");
    assert_eq!(simple_run("123\n456\n789", "Vd").await, "456\n789");
    assert_eq!(simple_run("123\n456\n789", "Vjd").await, "789");
    assert_eq!(
        simple_run("123\n456\n789", "Vyp").await,
        "123\n123\n456\n789"
    );
    // Visual S
    assert_eq!(simple_run("abc", "veS)").await, "(abc)");

    // Goto
    assert_eq!(
        simple_run("123\n456\n789", " g2\nix").await,
        "123\nx456\n789"
    );
}

#[tokio::test]
async fn test_hard_tab_setting() {
    use accepted::config::types::keys::HardTab;
    let syntax_parent = accepted::syntax::SyntaxParent::default();
    let mut config = config::ConfigWithDefault::default();
    config.set::<HardTab>(true);
    let buf: Buffer<RopeyCoreBuffer> = Buffer::new(&syntax_parent, &config);
    let state = BufferMode::new(buf);

    assert_eq!(state.buf.hard_tab(), true);

    assert_eq!(simple_run_config(&config, "", "i\t").await, "\t");
    assert_eq!(
        simple_run_config(&config, "", "i{\nabc").await,
        "{\n\tabc\n}"
    );
}

#[tokio::test]
async fn test_crlf() {
    assert_eq!(simple_run("a\r\na", "xxx").await, "");
    assert_eq!(simple_run("a\r\na", "dd").await, "a");
    assert_eq!(simple_run("ab\r\na", "vlld").await, "a");
}

#[tokio::test]
async fn fuzz_1() {
    test_from_fuzz(&[0x62, 0x25, 0xff, 0x29, 0x41, 0xff]).await;
}
