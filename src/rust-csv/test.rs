use csv = lib;
use csv::{rowreader, new_reader, new_reader_readlen, rowiter};

use std::str;
use std::io;
use io::Reader;
use io::mem::MemReader;
use std::vec;

mod lib;

    fn rowmatch(testdata: &'static str, expected: ~[~[&'static str]]) {
        let chk = |s: ~str, mk: proc(~Reader) -> ~rowreader| {
            let f = ~MemReader::new(s.into_bytes());
            let r = mk(f);
            let mut i = 0u;
            let mut row: ~[~str] = ~[];
            loop {
                let temp = r.readrow();
                if(temp.len() == 0) {
                  break;
                }
                row = vec::append(row, temp);
                let expect = expected[i];
                assert!(row.len() == expect.len());
                let mut j = 0u;
                while j < row.len() {
                    assert!(expect[j] == row[j]);
                    j += 1u;
                }
                i += 1u;
            }
            assert!(i == expected.len());
        };
        let runchecks = |s: &'static str| {
            // test default reader params
            do chk(s.to_owned()) |inp| {
                new_reader_readlen(inp, ',', '"', 2u)
            };
            // test default constructor
            do chk(s.to_owned()) |inp| {
                new_reader(inp, ',', '"')
            };
            // test continuations over read buffers
            let mut j = 1u;
            while j < s.len() {
                do chk(s.to_owned()) |inp| {
                    new_reader_readlen(inp, ',', '"', j)
                };
                j += 1u;
            }
            return;
        };
        // so we can test trailing newline case, testdata
        // must not end in \n - leave off the last newline
        runchecks(testdata);
        runchecks(str::replace(testdata, "\n", "\r\n"));
        if !testdata.ends_with("\n") {
            runchecks(testdata+"\n");
            runchecks(str::replace(testdata+"\n", "\n", "\r\n"));
        }
    }

    #[test]
    fn simple() {
        rowmatch("a,b,c,d\n1,2,3,4",
                 ~[~["a", "b", "c", "d"], ~["1", "2", "3", "4"]]);
    }

    #[test]
    fn trailing_comma() {
        rowmatch("a,b,c,d\n1,2,3,4,",
                 ~[~["a", "b", "c", "d"], ~["1", "2", "3", "4", ""]]);
    }

    #[test]
    fn leading_comma() {
        rowmatch("a,b,c,d\n,1,2,3,4",
                 ~[~["a", "b", "c", "d"], ~["", "1", "2", "3", "4"]]);
    }

    #[test]
    fn quote_simple() {
        rowmatch(~"\"Hello\",\"There\"\na,b,\"c\",d",
                 ~[~["Hello", "There"], ~["a", "b", "c", "d"]]);
    }

    #[test]
    fn quote_nested() {
        rowmatch(~"\"Hello\",\"There is a \"\"fly\"\" in my soup\"\na,b,\"c\",d",
                 ~[~["Hello", "There is a \"fly\" in my soup"], ~["a", "b", "c", "d"]]);
    }

    #[test]
    fn quote_with_comma() {
        rowmatch(~"\"1,2\"",
                 ~[~["1,2"]])
    }

    #[test]
    fn quote_with_other_comma() {
        rowmatch(~"1,2,3,\"a,b,c\"",
                 ~[~["1", "2", "3", "a,b,c"]])
    }

    #[test]
    fn blank_line() {
        rowmatch(~"\n\n", ~[~[], ~[]]);
    }

    #[test]
    fn iter_test() {
        let s = ~"a brown,cat";
        let f = ~MemReader::new(s.into_bytes());
        let r : ~rowreader = new_reader(f, ',', '"');
        for row in r.iter() {
            assert!(row[0] == "a brown");
            assert!(row[1] == "cat");
        }
    }

