extern crate defeat;
use defeat::Backtrace;

fn the_cause() {
    let bt = Backtrace::capture().unwrap().trimmed();
    println!("{}", bt);
}

fn foo() {
    the_cause();
}

fn main() {
    foo();
}
