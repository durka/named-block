// TODO
// - eliminate the closure hole by recognizing nested calls and shadowed labels, then maintaining a whitelist?
// - inline @up rule to reduce recursion depth

#![cfg_attr(not(test), no_std)]

// the tests need more recursion to parse all the code
#![cfg_attr(test, recursion_limit = "1000")]

// on nightly, we can re-export static_cond!
#![cfg_attr(feature = "nightly", feature(macro_reexport))]

#[cfg(all(test,                       // for testing ...
          not(feature = "nightly")))] // ... on beta/stable ...
#[macro_use]                          // ... we use the macros ...
#[no_link]                            // ... but no code ...
extern crate static_cond;             // ... from static-cond

#[cfg(feature = "nightly")]    // on nightly ...
#[macro_use]                   // ... we use the macros ...
#[no_link]                     // ... but no code ...
#[macro_reexport(static_cond)] // ... and re-export static-cond! ...
extern crate static_cond;      // ... from static-cond

/// Provides the "early exit from any block" control-flow primitive that was mentioned in [RFC 243][link].
///
/// If not using the "nightly" Cargo feature, you must depend on `static-cond` and put `#[macro_use] extern crate static_cond;` at the crate root.
///
/// See README.md for more details.
///
/// [link]: https://github.com/rust-lang/rfcs/blob/master/text/0243-trait-based-exception-handling.md#early-exit-from-any-block
///
/// Examples
/// ========
///
/// ```
/// # #[macro_use] extern crate named_block;
/// # #[macro_use] extern crate static_cond;
/// # fn main() {
/// let x = block!('a: {
///     break 'a 0;
///     1
/// });
/// assert_eq!(x, 0);
/// # }
/// ```
///
/// ```
/// # #[macro_use] extern crate named_block;
/// # #[macro_use] extern crate static_cond;
/// # fn main() {
/// assert_eq!(
///     42,
///     block!('a: {
///         enum Foo { Bar(i32) }
///         let closure = #[block(ignore)] {
///             move |Foo::Bar(x): Foo| -> i32 {
///                 x + block!('a: {
///                     break 'a 41;
///                 })
///             }
///         };
///     
///         closure(Foo::Bar(1))
///     })
/// );
/// # }
/// ```
#[macro_export]
macro_rules! block {
    // =======================================================================================
    // PRIVATE RULES
    // =======================================================================================
    
    // ======================================================
    // UTILITY RULES
    // ======================================================

    // utility: coerce an AST fragment with interpolated TTs into an expression
    // (see https://danielkeep.github.io/tlborm/book/blk-ast-coercion.html)
    (@as_expr $e:expr) => { $e };
    
    // utility: deliberately cause a compile error with a CamelCaseMessage
    (@error $err:ident) => {{
        struct $err;
        let _: () = $err;
    }};

    // ======================================================
    // OUTPUT STAGE
    // ======================================================
    // This is called from the scanner when everything has
    // been processed and we are ready to write out the
    // final expansion.
    
    // final output from the top level of the macro
    (@wrap $life:tt () $ret:ident ($($init:tt)*) $out:expr) => {
        block!(@as_expr
            {
                let $ret $($init)*;
                $life: loop {
                    $ret = $out;
                    break $life;
                }
                $ret
            })
    };
    (@wrap $life:tt (loop) $ret:ident ($($init:tt)*) $out:expr) => {
        block!(@as_expr
            {
                let $ret $($init)*;
                $life: loop {
                    $out;
                }
                $ret
            })
    };

    // ======================================================
    // SCANNER STAGE
    // ======================================================
    // This is the meat of the macro. It transfers code from
    // the input (left of ->) to the output (right of ->)
    // while transforming break statements or triggering
    // errors as needed.

    // The next four rules handle the end of the input code -- either the macro
    // is done, or we need to pop the stack and keep walking. We can tell which
    // it is by checking the context stack. If it's empty, we can go to output.
    // Otherwise, we take the current output, surround it by the brace type,
    // and move up the context stack.
    
    // no context: we're done!
    (@scan {} $life:tt $ret:ident () -> ($($out:tt)*) (() $lp:tt $init:tt)) => {
        block!(@wrap $life $lp $ret $init { $($out)* })
    };
    // pop stack and surround with {}
    (@scan {} $life:tt $ret:ident () -> ($($out:tt)*) $stack:tt) => {
        block!(@up $life $ret { $($out)* } $stack)
    };
    // pop stack and surround with ()
    (@scan () $life:tt $ret:ident () -> ($($out:tt)*) $stack:tt) => {
        block!(@up $life $ret ( $($out)* ) $stack)
    };
    // pop stack and surround with []
    (@scan [] $life:tt $ret:ident () -> ($($out:tt)*) $stack:tt) => {
        block!(@up $life $ret [ $($out)* ] $stack)
    };
    
    // The next nine rules are triggered when the tree walker encounters a
    // break/continue statement.

    // bare "break" and "continue" statements are errors (TODO allow bare break?)
    (@scan $paren:tt $life:tt $ret:ident (break) -> ($($out:tt)*) $stack:tt) => {
        block!(@scan $paren $life $ret () -> ($($out)* block!(@error NoBareBreakInNamedBlock);) $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (break; $($tail:tt)*) -> ($($out:tt)*) $stack:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* block!(@error NoBareBreakInNamedBlock);) $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (continue) -> ($($out:tt)*) $stack:tt) => {
        block!(@scan $paren $life $ret () -> ($($out)* block!(@error NoBareContinueInNamedBlock);) $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (continue; $($tail:tt)*) -> ($($out:tt)*) $stack:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* block!(@error NoBareContinueInNamedBlock);) $stack)
    };
    // "break LIFETIME;" (no EXPR)
    (@scan $paren:tt $life1:tt $ret:ident (break $life2:tt; $($tail:tt)*) -> ($($out:tt)*) $stack:tt) => {
        block!(@scan $paren $life1 $ret ($($tail)*) -> ($($out)* break $life2;) $stack)
    };
    // "break LIFETIME EXPR": compare the lifetimes, if they match then transform the statement, otherwise leave it alone
    (@scan $paren:tt $life1:tt $ret:ident (break $life2:tt $e:expr; $($tail:tt)*) -> ($($out:tt)*) ($stack:tt $lp:tt $init:tt)) => {
        static_cond! {
            if $life1 == $life2 {
                block!(@scan $paren $life1 $ret ($($tail)*) -> ($($out)* { $ret = $e; break $life2; }) ($stack $lp ()))
            } else {
                block!(@scan $paren $life1 $ret ($($tail)*) -> ($($out)* break $life2 $e;) ($stack $lp ()))
            }
        }
    };
    (@scan $paren:tt $life1:tt $ret:ident (break $life2:tt $e:expr) -> ($($out:tt)*) ($stack:tt $lp:tt $init:tt)) => {
        static_cond! {
            if $life1 == $life2 {
                block!(@scan $paren $life1 $ret () -> ($($out)* { $ret = $e; break $life2 }) ($stack $lp ()))
                    // TODO make sure this isn't adding too many semicolons
            } else {
                block!(@scan $paren $life1 $ret () -> ($($out)* break $life2 $e;) ($stack $lp ()))
            }
        }
    };
    // "continue LIFETIME": compare the lifetimes, if they match then error, otherwise leave it alone
    // (this only applies to bare blocks)
    (@scan $paren:tt $life1:tt $ret:ident (continue $life2:tt; $($tail:tt)*) -> ($($out:tt)*) ($stack:tt () $init:tt)) => {
        static_cond! {
            if $life1 == $life2 {
                block!(@scan $paren $life1 $ret ($($tail)*) -> ($($out)* block!(@error NoMatchedContinueInNamedBlock);) ($stack () $init))
            } else {
                block!(@scan $paren $life1 $ret ($($tail)*) -> ($($out)* continue $life2;) ($stack () $init))
            }
        }
    };
    (@scan $paren:tt $life1:tt $ret:ident (continue $life2:tt) -> ($($out:tt)*) $stack:tt) => {
        static_cond! {
            if $life1 == $life2 {
                block!(@scan $paren $life1 $ret () -> ($($out)* block!(@error NoMatchedContinueInNamedBlock);) $stack)
            } else {
                block!(@scan $paren $life1 $ret () -> ($($out)* continue $life2;) $stack)
            }
        }
    };

    // tree walker ignores #[block(ignore)] tts, closures, and items
    
    (@scan_item $paren:tt $life:tt $ret:ident ($ignore:item $($tail:tt)*) -> ($($out:tt)*) $stack:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* $ignore) $stack)
    };
    
    // #[block(ignore)] attribute is ignored
    (@scan $paren:tt $life:tt $ret:ident (#[block(ignore)] $ignore:tt $($tail:tt)*) -> ($($out:tt)*) $stack:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* $ignore) $stack)
    };
    // other attributes pass through
    (@scan $paren:tt $life:tt $ret:ident (#[$attr:meta] $($tail:tt)*) -> ($($out:tt)*) $stack:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* #[$attr]) $stack)
    };
    // ignore items: use, extern, static, const, unsafe trait/impl/fn, fn, mod, type, enum, trait, impl, struct
    (@scan $paren:tt $life:tt $ret:ident (pub $($tail:tt)*) -> $out:tt $stack:tt) => {
        block!(@scan_item $paren $life $ret (pub $($tail)*) -> $out $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (use $($tail:tt)*) -> $out:tt $stack:tt) => {
        block!(@scan_item $paren $life $ret (use $($tail)*) -> $out $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (extern $($tail:tt)*) -> $out:tt $stack:tt) => {
        block!(@scan_item $paren $life $ret (extern $($tail)*) -> $out $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (mod $($tail:tt)*) -> $out:tt $stack:tt) => {
        block!(@scan_item $paren $life $ret (mod $($tail)*) -> $out $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (static $($tail:tt)*) -> $out:tt $stack:tt) => {
        block!(@scan_item $paren $life $ret (static $($tail)*) -> $out $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (const $($tail:tt)*) -> $out:tt $stack:tt) => {
        block!(@scan_item $paren $life $ret (const $($tail)*) -> $out $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (trait $($tail:tt)*) -> $out:tt $stack:tt) => {
        block!(@scan_item $paren $life $ret (trait $($tail)*) -> $out $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (unsafe trait $($tail:tt)*) -> $out:tt $stack:tt) => {
        block!(@scan_item $paren $life $ret (unsafe trait $($tail)*) -> $out $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (impl $($tail:tt)*) -> $out:tt $stack:tt) => {
        block!(@scan_item $paren $life $ret (impl $($tail)*) -> $out $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (unsafe impl $($tail:tt)*) -> $out:tt $stack:tt) => {
        block!(@scan_item $paren $life $ret (unsafe impl $($tail)*) -> $out $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (fn $($tail:tt)*) -> $out:tt $stack:tt) => {
        block!(@scan_item $paren $life $ret (fn $($tail)*) -> $out $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (unsafe fn $($tail:tt)*) -> $out:tt $stack:tt) => {
        block!(@scan_item $paren $life $ret (unsafe fn $($tail)*) -> $out $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (type $($tail:tt)*) -> $out:tt $stack:tt) => {
        block!(@scan_item $paren $life $ret (type $($tail)*) -> $out $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (enum $($tail:tt)*) -> $out:tt $stack:tt) => {
        block!(@scan_item $paren $life $ret (enum $($tail)*) -> $out $stack)
    };
    (@scan $paren:tt $life:tt $ret:ident (struct $($tail:tt)*) -> $out:tt $stack:tt) => {
        block!(@scan_item $paren $life $ret (struct $($tail)*) -> $out $stack)
    };
    
    // tree walker descends into token trees
    (@scan $paren:tt $life:tt $ret:ident ({ $($inner:tt)* } $($tail:tt)*) -> $out:tt ($stack:tt $lp:tt $init:tt)) => {
        block!(@scan {} $life $ret ($($inner)*) -> ()
               (($paren ($($tail)*) -> $out $stack) $lp $init))
    };
    (@scan $paren:tt $life:tt $ret:ident (( $($inner:tt)* ) $($tail:tt)*) -> $out:tt ($stack:tt $lp:tt $init:tt)) => {
        block!(@scan () $life $ret ($($inner)*) -> ()
               (($paren ($($tail)*) -> $out $stack) $lp $init))
    };
    (@scan $paren:tt $life:tt $ret:ident ([ $($inner:tt)* ] $($tail:tt)*) -> $out:tt ($stack:tt $lp:tt $init:tt)) => {
        block!(@scan [] $life $ret ($($inner)*) -> ()
               (($paren ($($tail)*) -> $out $stack) $lp $init))
    };

    // fall-through case for tree walker: transfer over a token
    (@scan $paren:tt $life:tt $ret:ident ($head:tt $($tail:tt)*) -> ($($out:tt)*) $stack:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* $head) $stack)
    };

    // reformats arguments when popping a context off the tree walker stack
    // TODO this could be folded into the @scan rules that call it, to reduce recursion depth
    (@up $life:tt $ret:ident $thing:tt (($paren:tt $tail:tt -> ($($out:tt)*) $stack:tt) $lp:tt $init:tt)) => {
        block!(@scan $paren $life $ret $tail -> ($($out)* $thing) ($stack $lp $init))
    };

    // entry point for bare block
    ($life:tt: { $($body:tt)* }) => {
        block!(@scan {} $life _ret ($($body)*) -> () (() () ()))
        //      |    |  |     |    |              |  ||  |  |
        //      |    |  |     |    |              |  ||  |  ^ initialization
        //      |    |  |     |    |              |  ||  ^ loop type
        //      |    |  |     |    |              |  |^ tree walker stack
        //      |    |  |     |    |              |  ^ passed-through context
        //      |    |  |     |    |              ^ transformed code
        //      |    |  |     |    ^ code to be transformed
        //      |    |  |     ^ block exit variable name (gensym)
        //      |    |  ^ block label
        //      |    ^ surrounding bracket type
        //      ^ start the tree walker!
    };

    // entry point for loop
    ($life:tt: loop { $($body:tt)* }) => {
        block!(@scan {} $life _ret ($($body)*) -> () (() (loop) (= ())))
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let flag = true;
        let x = block!('a: {
            if flag { break 'a "early exit"; }
            "normal exit"
        });
        assert_eq!(x, "early exit");
    }

    #[test]
    fn shadowing() {
        let flag = false;
        let x = block!('b: {
            if flag { break 'b "early exit"; }
            let _y = block!('c: {
                if flag { break 'b "inner early exit"; };
                String::from("inner normal exit")
            });

            #[block(ignore)]
            {
                #[allow(dead_code)]
                fn f() -> i32 {
                    block!('b: {
                        break 'b 42;
                    })
                }
            }
            #[allow(dead_code)]
            fn g() {
                block!('b: {
                    break 'b 42;
                });

                while false {
                    break;
                }
                while false {
                    continue;
                }
                'b: while false {
                    continue 'b;
                }
            }

            enum Foo { Bar(i32) }
            let closure = move |Foo::Bar(x): Foo| -> i32 {
                x + block!('d: {
                    break 'd 42;
                })
            };
            assert_eq!(closure(Foo::Bar(0)), 42);

            "normal exit"
        });
        assert_eq!(x, "normal exit");

        'e: for i in 1..5 {
            assert!(i >= 1 && i < 5);
            block!('d: {
                //continue; //~ERROR NoBareContinueInNamedBlock
                //continue 'd; //~ERROR NoMatchedContinueInNamedBlock
                continue 'e;
            });
        }
    }

    #[test]
    fn loops() {
        assert_eq!(42, block!('a: loop { break 'a 42 }));
        assert_eq!((), block!('a: {})); // make sure it works with no breaks

        let mut v = vec![];
        let mut i = 0;
        block!('a: loop {
            i += 1;
            if i == 5 {
                continue 'a;
            } else if i == 10 {
                break 'a;
            }
            v.push(i);
        });
        assert_eq!(&*v, &[1, 2, 3, 4, 6, 7, 8, 9]);
    }
}

