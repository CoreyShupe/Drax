use proc_macro2::{token_stream::IntoIter, Delimiter, Ident, Span, TokenStream, TokenTree};

fn read_punct(tokens: &mut IntoIter, c: char) {
    if let Some(next) = tokens.next() {
        if let TokenTree::Punct(next) = &next {
            if next.as_char() == c {
                return;
            }
        }
    }
    panic!("Expected punct {}", c);
}

fn read_sep(tokens: &mut IntoIter) {
    read_punct(tokens, '-');
    read_punct(tokens, '>');
}

fn read_to_next_comma(tokens: &mut IntoIter) -> TokenStream {
    let mut inner = Vec::new();
    while let Some(next) = tokens.next() {
        if let TokenTree::Punct(next) = &next {
            if next.as_char() == ',' {
                break;
            }
        }
        inner.push(next);
    }
    inner.into_iter().collect::<TokenStream>()
}

fn read_list_unknown(tokens: &mut IntoIter) -> TokenStream {
    unimplemented!()
}

pub(crate) fn read_tag_inner_internal(tokens: &mut IntoIter) -> proc_macro::TokenStream {
    read_tag_inner(tokens, true)
}

fn read_tag_inner(tokens: &mut IntoIter, initial: bool) -> proc_macro::TokenStream {
    let tag_ident = Ident::new("tag", Span::call_site());
    let mut tag_pushers = Vec::new();
    while let Some(next) = tokens.next() {
        match next {
            TokenTree::Literal(literal) => {
                read_sep(tokens);
                let inner_maker = match tokens.next() {
                    Some(TokenTree::Group(group)) => {
                        if matches!(group.delimiter(), Delimiter::Brace) {
                            let ret = read_tag_inner(&mut group.stream().into_iter(), false).into();
                            read_punct(tokens, ',');
                            ret
                        } else {
                            panic!("Unexpected group {group:?}.");
                        }
                    }
                    tr => {
                        let all = read_to_next_comma(tokens);
                        quote::quote!((#tr #all).into())
                    }
                };
                tag_pushers.push(quote::quote! {
                    #tag_ident.put_tag(#literal, #inner_maker);
                });
            }
            tr => panic!("Expected literal, found: {:?}", tr),
        }
    }

    if initial {
        proc_macro::TokenStream::from(quote::quote! {
            {
                let mut #tag_ident = drax::nbt::CompoundTag::new();
                #(#tag_pushers)*
                tag
            }
        })
    } else {
        proc_macro::TokenStream::from(quote::quote! {
            {
                let mut #tag_ident = drax::nbt::CompoundTag::new();
                #(#tag_pushers)*
                drax::nbt::Tag::CompoundTag(tag)
            }
        })
    }
}
