#![feature(test)]

extern crate test;

use test::Bencher;

use termion::event::{Event, Key};

use accepted::{config, Buffer, BufferMode};

#[bench]
fn bench_insert_1(b: &mut Bencher) {
    let syntax_parent = accepted::syntax::SyntaxParent::default();
    let buf = Buffer::new(&syntax_parent, config::ConfigWithDefault::default());
    let mut state = BufferMode::new(buf);

    state.event(Event::Key(Key::Char('i')));
    b.iter(move || {
        state.event(Event::Key(Key::Char('a')));
    });
}
