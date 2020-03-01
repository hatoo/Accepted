#![feature(test)]

extern crate test;

use test::Bencher;

use termion::event::{Event, Key};

use accepted::{config, core::buffer::RopeyCoreBuffer, Buffer, BufferMode};

#[bench]
#[tokio::main]
async fn bench_insert_1(b: &mut Bencher) {
    let syntax_parent = accepted::syntax::SyntaxParent::default();
    let config = config::ConfigWithDefault::default();
    let buf = Buffer::new(&syntax_parent, &config);
    let mut state = BufferMode::<RopeyCoreBuffer>::new(buf);

    state.event(Event::Key(Key::Char('i')));
    b.iter(move || {
        state.event(Event::Key(Key::Char('a')));
    });
}
