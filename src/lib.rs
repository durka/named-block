// TODO
// - comment macro
// - support loops?

#![recursion_limit = "1000"]

/// https://github.com/rust-lang/rfcs/blob/master/text/0243-trait-based-exception-handling.md#early-exit-from-any-block
///
/// Examples
/// ========
///
/// ```
/// # #[macro_use] extern crate named_block;
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
/// # fn main() {
/// assert_eq!(
///     42,
///     block!('a: {
///         enum Foo { Bar(i32) }
///         let closure = #[block(ignore)] {
///             move |Foo::Bar(x): Foo| -> i32 {
///                 x + block!('a: {
///                     break 'a 42;
///                 })
///             }
///         };
///     
///         closure(Foo::Bar(0))
///     })
/// );
/// # }
/// ```
#[macro_export]
macro_rules! block {
    // utility for AST coercion
    (@as_expr $e:expr) => { $e };
    
    // cause a compile error with a CamelCaseMessage
    (@error $err:ident) => {{
        struct $err;
        let _: () = $err;
    }};
    
    // final output from the top level of the macro
    (@wrap $life:tt $ret:ident $out:expr) => {
        block!(@as_expr
            {
                let $ret;
                $life: loop {
                    $ret = $out;
                    break $life;
                }
                $ret
            })
    };

    // handle the end of the input code -- either the macro is done, or pop the stack and keep walking
    
    // no context: we're done!
    (@scan {} $life:tt $ret:ident () -> ($($out:tt)*) ()) => {
        block!(@wrap $life $ret { $($out)* })
    };
    // pop stack and surround with {}
    (@scan {} $life:tt $ret:ident () -> ($($out:tt)*) $ctx:tt) => {
        block!(@up $life $ret { $($out)* } $ctx)
    };
    // pop stack and surround with ()
    (@scan () $life:tt $ret:ident () -> ($($out:tt)*) $ctx:tt) => {
        block!(@up $life $ret ( $($out)* ) $ctx)
    };
    // pop stack and surround with []
    (@scan [] $life:tt $ret:ident () -> ($($out:tt)*) $ctx:tt) => {
        block!(@up $life $ret [ $($out)* ] $ctx)
    };
    
    // tree walker encounters a break/continue statement

    // bare "break" and "continue" statements are errors (TODO allow bare break?)
    (@scan $paren:tt $life:tt $ret:ident (break) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life $ret () -> ($($out)* block!(@error NoBareBreakInNamedBlock);) $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (break; $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* block!(@error NoBareBreakInNamedBlock);) $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (continue) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life $ret () -> ($($out)* block!(@error NoBareContinueInNamedBlock);) $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (continue; $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* block!(@error NoBareContinueInNamedBlock);) $ctx)
    };
    // "break LIFETIME;" (no EXPR)
    (@scan $paren:tt $life1:tt $ret:ident (break $life2:tt; $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life1 $ret ($($tail)*) -> ($($out)* break $life2;) $ctx)
    };
    // "break LIFETIME EXPR": compare the lifetimes, if they match then transform the statement, otherwise leave it alone
    (@scan $paren:tt $life1:tt $ret:ident (break $life2:tt $e:expr; $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {{
        // this is the trick for comparing tokens: generate a fresh macro and call it
        macro_rules! __block_check {
            ($life1 $life1) => { // equal
                block!(@scan $paren $life1 $ret ($($tail)*) -> ($($out)* { $ret = $e; break $life2; }) $ctx)
            };
            ($life1 $life2) => { // unequal
                block!(@scan $paren $life1 $ret ($($tail)*) -> ($($out)* break $life2 $e;) $ctx)
            };
        }
        
        __block_check!($life1 $life2)
    }};
    (@scan $paren:tt $life1:tt $ret:ident (break $life2:tt $e:expr) -> ($($out:tt)*) $ctx:tt) => {{
        macro_rules! __block_check {
            ($life1 $life1) => { // equal
                block!(@scan $paren $life1 $ret () -> ($($out)* { $ret = $e; break $life2 }) $ctx)
                    // TODO make sure this isn't adding too many semicolons
            };
            ($life1 $life2) => { // unequal
                block!(@scan $paren $life1 $ret () -> ($($out)* break $life2 $e;) $ctx)
            };
        }
        
        __block_check!($life1 $life2)
    }};
    // "continue LIFETIME": compare the lifetimes, if they match then error, otherwise leave it alone
    (@scan $paren:tt $life1:tt $ret:ident (continue $life2:tt; $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {{
        macro_rules! __block_check {
            ($life1 $life1) => { // equal
                block!(@scan $paren $life1 $ret ($($tail)*) -> ($($out)* block!(@error NoMatchedContinueInNamedBlock);) $ctx)
            };
            ($life1 $life2) => { // unequal
                block!(@scan $paren $life1 $ret ($($tail)*) -> ($($out)* continue $life2;) $ctx)
            };
        }
        
        __block_check!($life1 $life2)
    }};
    (@scan $paren:tt $life1:tt $ret:ident (continue $life2:tt) -> ($($out:tt)*) $ctx:tt) => {{
        macro_rules! __block_check {
            ($life1 $life1) => { // equal
                block!(@scan $paren $life1 $ret () -> ($($out)* block!(@error NoMatchedContinueInNamedBlock);) $ctx)
            };
            ($life1 $life2) => { // unequal
                block!(@scan $paren $life1 $ret () -> ($($out)* continue $life2;) $ctx)
            };
        }
        
        __block_check!($life1 $life2)
    }};

    // tree walker ignores #[block(ignore)] tts, closures, and items
    
    (@scan_item $paren:tt $life:tt $ret:ident ($ignore:item $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* $ignore) $ctx)
    };
    
    // #[block(ignore)] attribute is ignored
    (@scan $paren:tt $life:tt $ret:ident (#[block(ignore)] $ignore:tt $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* $ignore) $ctx)
    };
    // other attributes pass through
    (@scan $paren:tt $life:tt $ret:ident (#[$attr:meta] $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* #[$attr]) $ctx)
    };
    // ignore items: use, extern, static, const, unsafe trait/impl/fn, fn, mod, type, enum, trait, impl, struct
    (@scan $paren:tt $life:tt $ret:ident (use $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (use $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (extern $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (extern $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (mod $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (mod $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (static $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (static $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (const $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (const $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (trait $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (trait $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (unsafe trait $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (unsafe trait $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (impl $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (impl $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (unsafe impl $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (unsafe impl $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (fn $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (fn $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (unsafe fn $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (unsafe fn $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (type $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (type $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (enum $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (enum $($tail)*) -> $out $ctx)
    };
    (@scan $paren:tt $life:tt $ret:ident (struct $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan_item $paren $life $ret (struct $($tail)*) -> $out $ctx)
    };
    
    // tree walker descends into token trees
    (@scan $paren:tt $life:tt $ret:ident ({ $($inner:tt)* } $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan {} $life $ret ($($inner)*) -> ()
               ($paren ($($tail)*) -> $out $ctx))
    };
    (@scan $paren:tt $life:tt $ret:ident (( $($inner:tt)* ) $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan () $life $ret ($($inner)*) -> ()
               ($paren ($($tail)*) -> $out $ctx))
    };
    (@scan $paren:tt $life:tt $ret:ident ([ $($inner:tt)* ] $($tail:tt)*) -> $out:tt $ctx:tt) => {
        block!(@scan [] $life $ret ($($inner)*) -> ()
               (($($tail)*) -> $out))
    };

    // fall-through case for tree walker: transfer over a token
    (@scan $paren:tt $life:tt $ret:ident ($head:tt $($tail:tt)*) -> ($($out:tt)*) $ctx:tt) => {
        block!(@scan $paren $life $ret ($($tail)*) -> ($($out)* $head) $ctx)
    };

    // reformats arguments when popping a context off the tree walker stack
    // TODO this could be folded into the @scan rules that call it, to reduce recursion depth
    (@up $life:tt $ret:ident $thing:tt ($paren:tt $tail:tt -> ($($out:tt)*) $ctx:tt)) => {
        block!(@scan $paren $life $ret $tail -> ($($out)* $thing) $ctx)
    };
    
    ($life:tt: { $($body:tt)* }) => {
        block!(@scan {} $life _ret ($($body)*) -> () ())
        //      |    |  |     |    |              |  |
        //      |    |  |     |    |              |  ^ tree walker stack
        //      |    |  |     |    |              ^ transformed code
        //      |    |  |     |    ^ code to be transformed
        //      |    |  |     ^ block exit variable name (gensym)
        //      |    |  ^ block label
        //      |    ^ surrounding bracket type
        //      ^ start the tree walker!
    }
}

mod tests {
    #![allow(warnings)]

    #[test]
    fn it_works() {
        let flag = true;
        let x = block!('a: {
            if flag { break 'a "early exit"; }
            "normal exit"
        });
        assert_eq!(x, "early exit");

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
}

