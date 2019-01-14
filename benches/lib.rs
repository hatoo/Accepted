#![feature(test)]

extern crate test;

use accepted::{Buffer, BufferMode};
use termion::event::{Event, Key};
use test::Bencher;

#[bench]
fn bench_insert_1(b: &mut Bencher) {
    let syntax_parent = accepted::syntax::SyntaxParent::default();
    let buf = Buffer::new(&syntax_parent);
    let mut state = BufferMode::new(buf);

    state.event(Event::Key(Key::Char('i')));
    b.iter(move || {
        state.event(Event::Key(Key::Char('a')));
    });
}
