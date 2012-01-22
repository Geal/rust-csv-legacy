
use std;
import std::io;

use csv;
import csv::{rowaccess,rowiter};

fn main(args : [str])
{
    if (vec::len(args) != 2u) {
        ret;
    }
    let f : io::reader = result::get(io::file_reader(args[1]));
    let reader = csv::new_reader_readlen(f, ',', '"', 1u);
    while true {
        let res = reader.readrow();
        if result::failure(res) {
            break;
        }
        let row = result::get(res);
        io::println(#fmt("---- ROW %u fields -----", row.len()));
        let i = 0u;
        while i < row.len() {
            io::println(row.getstr(i));
            i = i + 1u;
        }
    }
}

