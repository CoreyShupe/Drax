use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::{enum_packet_components, struct_packet_components};
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Size {
    Dynamic(usize),
    Constant(usize),
}

impl std::ops::Add for Size {
    type Output = Size;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Size::Dynamic(x), Size::Dynamic(y))
            | (Size::Dynamic(x), Size::Constant(y))
            | (Size::Constant(x), Size::Dynamic(y)) => Size::Dynamic(x + y),
            (Size::Constant(x), Size::Constant(y)) => Size::Constant(x + y),
        }
    }
}

impl std::ops::Add<usize> for Size {
    type Output = Size;

    fn add(self, rhs: usize) -> Self::Output {
        match self {
            Size::Dynamic(x) | Size::Constant(x) => Size::Dynamic(x + rhs),
        }
    }
}

/// Defines a structure that can be encoded and decoded.
pub trait PacketComponent<C> {
    type ComponentType: Sized;

    /// Decodes the packet component from the given reader.
    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        context: &'a mut C,
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>;

    /// Encodes the packet component to the given writer.
    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        context: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>>;

    fn size(input: &Self::ComponentType, context: &mut C) -> crate::prelude::Result<Size>;
}

macro_rules! impl_deref_component {
    ($c_ty:ty, $t_ty:ty) => {
        type ComponentType = <$t_ty as $crate::prelude::PacketComponent<$c_ty>>::ComponentType;

        fn decode<'a, A: $crate::prelude::AsyncRead + Unpin + ?Sized>(
            context: &'a mut $c_ty,
            read: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>> {
            <$t_ty as $crate::prelude::PacketComponent<$c_ty>>::decode(context, read)
        }

        fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
            component_ref: &'a Self::ComponentType,
            context: &'a mut $c_ty,
            write: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
            <$t_ty as $crate::prelude::PacketComponent<$c_ty>>::encode(
                component_ref,
                context,
                write,
            )
        }

        fn size(input: &Self::ComponentType, context: &mut $c_ty) -> crate::prelude::Result<Size> {
            <$t_ty as $crate::prelude::PacketComponent<$c_ty>>::size(input, context)
        }
    };
}

impl<T, C> PacketComponent<C> for Box<T>
where
    T: PacketComponent<C>,
{
    impl_deref_component!(C, T);
}

impl<T, C> PacketComponent<C> for Arc<T>
where
    T: PacketComponent<C>,
{
    impl_deref_component!(C, T);
}

impl<T, C> PacketComponent<C> for &T
where
    T: PacketComponent<C>,
{
    impl_deref_component!(C, T);
}

#[cfg(feature = "nbt")]
pub mod nbt;
pub mod option;
pub mod primitive;
#[cfg(feature = "serde")]
pub mod serde_json;
pub mod string;
pub mod vec;

#[cfg(feature = "macros")]
pub mod macros {
    #[macro_export]
    macro_rules! enum_packet_components {
        (@internal @match $key_ident:ident) => {
            $key_ident
        };
        (@internal @match $__:ident @alt $matcher:expr) => {
            $matcher
        };
        (@internal @case $value:literal) => {
            $value
        };
        (@internal @case $__:literal @alt $value:literal) => {
            $value
        };
        (@internal @vdoc $value:literal) => {
            stringify!($value)
        };
        (@internal @vdoc $__:literal @alt $value:literal) => {
            stringify!($value)
        };
        ($context:ident: $ctx_ty:ty, $w_ident:ident, $field_name:ident @ser : $ty:ty) => {
            $crate::expand_field!(@internal @ser_bind $context: $ctx_ty, $w_ident, $field_name, $ty)
        };
        ($context:ident: $ctx_ty:ty, $w_ident:ident, $field_name:ident @ser : $__:ty : $dty:ty) => {
            $crate::expand_field!(@internal @ser_bind $context: $ctx_ty, $w_ident, $field_name, $dty)
        };
        ($context:ident: $ctx_ty:ty, $c_counter:ident, $d_counter:ident, $field_name:ident @size : $ty:ty) => {
            $crate::expand_field!(@internal @size_bind $context: $ctx_ty, $c_counter, $d_counter, $field_name, $ty)
        };
        ($context:ident: $ctx_ty:ty, $c_counter:ident, $d_counter:ident, $field_name:ident @size : $__:ty : $dty:ty) => {
            $crate::expand_field!(@internal @size_bind $context: $ctx_ty, $c_counter, $d_counter, $field_name, $dty)
        };
        ($($(#[$($tt:tt)*])* $enum_name:ident$(<$ctx_ty:ty>)? {
            $key_name:ident: $key_delegate_type:ty,
                $(@ser_delegate $static_product_delegate_type:ty,)?
                $(@match $key_matcher:expr,)?
            $(
                $(#[$($variant_tt:tt)*])*
                $($key_matcher_case:literal =>)? $variant_name:ident {
                    $(
                        $(
                            $(#[$($doc_tt:tt)*])*
                            $field_name:ident: $delegate_type:ty,
                        )+
                    )?
                }
            ),*
        })*) => {$(
            macro_rules! ctx_type {
                ($$alt_ty:ty) => {
                    $crate::expand_field!(@internal @ty_bind $$alt_ty; $(@alt $ctx_ty)?)
                };
            }

            $(#[$($tt)*])*
            ///
            /// Packet Field Explanation
            /// ---
            /// <table>
            /// <thead>
            ///     <tr>
            ///         <th>Key</th>
            ///         <th>Variant</th>
            ///         <th>Description</th>
            ///     </tr>
            /// </thead>
            /// <tbody>
                $(
                    /// <tr>
                    ///   <td>
                    #[doc=$crate::enum_packet_components!(@internal @vdoc ${index(0)} $(@alt $key_matcher_case)?)]
                    ///   </td>
                    ///   <td>
                    #[doc=stringify!($variant_name)]
                    ///   </td>
                    ///   <td>
                    #[doc=$crate::expand_field!(@internal @doc $(#[$($variant_tt)*])*)]
                    $(#[$($variant_tt)*])*
                    ///   </td>
                    $(
                    ///   <td>
                    #[doc="<table><thead><tr><th>Field</th><th>Description</th></tr></thead><tbody>"]
                    $(
                    #[doc=concat!(
                        "<tr><td>",
                        stringify!($field_name),
                        "</td><td>"
                    )]
                    #[doc=$crate::expand_field!(@internal @doc $(#[$($doc_tt)*])*)]
                    $(#[$($doc_tt)*])*
                    #[doc="</td></tr>"]
                    )+
                    #[doc="</tbody></table>"]
                    /// </td>
                    )?
                    /// </tr>
                )*
            /// </tbody>
            /// </table>
            pub enum $enum_name {
                $(
                    $variant_name$({
                        $(
                        $field_name: <$delegate_type as $crate::transport::packet::PacketComponent<ctx_type!(())>>::ComponentType,
                        )+
                    })?,
                )*
            }

            $crate::expand_field!(@internal @impl_bind $enum_name, C $(@alt $ctx_ty)? {
                type ComponentType = Self;

                fn decode<'a, A: $crate::prelude::AsyncRead + Unpin + ?Sized>(
                    __context: &'a mut ctx_type!(C),
                    __read: &'a mut A,
                ) -> std::pin::Pin<Box<dyn std::future::Future<Output = $crate::transport::Result<Self>> + 'a>>
                where
                    Self: Sized
                {
                    Box::pin(async move {
                        $crate::expand_field!(@internal @de_bind __context: ctx_type!(C), __read, $key_name, $key_delegate_type);

                        match $crate::enum_packet_components!(@internal @match $key_name $(@alt $key_matcher)?) {
                            $(
                            $crate::enum_packet_components!(@internal @case ${index(0)} $(@alt $key_matcher_case)?) => {
                                $($(
                                    $crate::expand_field!(@internal @de_bind __context: ctx_type!(C), __read, $field_name, $delegate_type);
                                )+)?
                                Ok(Self::$variant_name $({
                                    $($field_name,)*
                                })?)
                            }
                            )*
                            _ => $crate::throw_explain!(format!("Failed to decode key {} for type {}", $key_name, stringify!($enum_name))),
                        }
                    })
                }

                fn encode<'a, A: $crate::prelude::AsyncWrite + Unpin + ?Sized>(
                    component_ref: &'a Self,
                    __context: &'a mut ctx_type!(C),
                    __write: &'a mut A,
                ) -> std::pin::Pin<Box<dyn std::future::Future<Output = $crate::transport::Result<()>> + 'a>>
                {
                    Box::pin(async move {
                        macro_rules! expand_key_types {
                            (
                                $$write_ref:ident,
                                $$key_ref:ident,
                                $$ctx_ref:ident
                            ) => {
                                $crate::enum_packet_components! {
                                    $$ctx_ref: ctx_type!(C), $$write_ref, $$key_ref @ser
                                    : $key_delegate_type
                                    $(: $static_product_delegate_type)?
                                }
                            }
                        }

                        match component_ref {
                            $(
                                Self::$variant_name $({$(
                                    $field_name,
                                )+})? => {
                                    {
                                        let key = $crate::enum_packet_components!(@internal @case ${index(0)} $(@alt $key_matcher_case)?);
                                        let key_ref = &key;
                                        expand_key_types!(__write, key_ref, __context);
                                    }
                                    $($(
                                        $crate::expand_field!(@internal @ser_bind __context: ctx_type!(C), __write, $field_name, $delegate_type);
                                    )+)?
                                    Ok(())
                                }
                            )*
                        }
                    })
                }

                fn size(component_ref: &Self, __context: &mut ctx_type!(C)) -> $crate::prelude::Result<$crate::prelude::Size>
                {
                    macro_rules! expand_key_types {
                        (
                            $$constant_counter:ident,
                            $$dynamic_counter:ident,
                            $$key_ref:ident,
                            $$ctx_ref:ident
                        ) => {
                            $crate::enum_packet_components! {
                                $$ctx_ref: ctx_type!(C), $$constant_counter, $$dynamic_counter, $$key_ref @size
                                : $key_delegate_type
                                $(: $static_product_delegate_type)?
                            }
                        }
                    }

                    let mut constant_counter = 0;
                    let mut dynamic_counter = 0;
                    match component_ref {
                        $(
                        Self::$variant_name $({$(
                        $field_name,
                        )+})? => {
                            {
                                let key = $crate::enum_packet_components!(@internal @case ${index(0)} $(@alt $key_matcher_case)?);
                                let key_ref = &key;
                                expand_key_types!(constant_counter, dynamic_counter, key_ref, __context);
                            }
                            $($(
                            $crate::expand_field!(@internal @size_bind __context: ctx_type!(C), constant_counter, dynamic_counter, $field_name, $delegate_type);
                            )+)?
                        }
                        )*
                    }

                    if constant_counter == dynamic_counter {
                        Ok($crate::transport::packet::Size::Constant(constant_counter))
                    } else {
                        Ok($crate::transport::packet::Size::Dynamic(dynamic_counter))
                    }
                }
            });
        )*};
    }

    #[macro_export]
    macro_rules! expand_field {
        (@internal @impl_bind $struct_name:ident, $field_name:ident { $($impl_tokens:tt)* }) => {
            impl<$field_name> $crate::transport::packet::PacketComponent<$field_name> for $struct_name {
                $($impl_tokens)*
            }
        };
        (@internal @impl_bind $struct_name:ident, $__:ident @alt $ctx_ty:ty { $($impl_tokens:tt)* }) => {
            impl $crate::transport::packet::PacketComponent<$ctx_ty> for $struct_name {
                $($impl_tokens)*
            }
        };
        (@internal @ty_bind $typing:ty;) => {
            $typing
        };
        (@internal @ty_bind $__:ty; @alt $ctx_ty:ty) => {
            $ctx_ty
        };
        (@internal @ser_bind $context:ident: $ctx_ty:ty, $w_ident:ident, $field_name:ident, $delegate_type:ty) => {
            <$delegate_type as $crate::transport::packet::PacketComponent<$ctx_ty>>::encode($field_name, $context, $w_ident).await?
        };
        (@internal @de_bind $context:ident: $ctx_ty:ty, $r_ident:ident, $field_name:ident, $delegate_type:ty) => {
            let $field_name = <$delegate_type as $crate::transport::packet::PacketComponent<$ctx_ty>>::decode($context, $r_ident).await?;
        };
        (@internal @size_bind $context:ident: $ctx_ty:ty, $c_counter:ident, $d_counter:ident, $field_name:ident, $delegate_type:ty) => {
            match <$delegate_type as $crate::transport::packet::PacketComponent<$ctx_ty>>::size($field_name, $context)?
            {
                $crate::transport::packet::Size::Constant(x) => {
                    $c_counter += x;
                    $d_counter += x;
                }
                $crate::transport::packet::Size::Dynamic(x) => $d_counter += x,
            }
        };
        (@internal @doc) => {
            "N/A"
        };
        (@internal @doc $(#[$($doc_tt:tt)*])*) => {
            ""
        };
    }

    #[macro_export]
    macro_rules! struct_packet_components {
        (@internal $(#[$($tt:tt)*])* @ $struct_name:ident) => {
            $(#[$($tt)*])*
            pub struct $struct_name;
        };
        (@internal $(#[$($tt:tt)*])* @expand {$($ctx_ty_tt:tt)+} $(
            $(@describe($description:expr))?
            $field_name:ident: $delegate_type:ty,
        )+ @ $struct_name:ident) => {
            macro_rules! ctx_type_struct {
                () => {
                    $($ctx_ty_tt)+
                };
            }

            $(#[$($tt)*])*
            pub struct $struct_name {
                $(
                pub $field_name: <$delegate_type as $crate::transport::packet::PacketComponent<ctx_type_struct!()>>::ComponentType,
                )+
            }
        };
        ($(
            $(#[$($tt:tt)*])*
            $struct_name:ident$(<$ctx_ty:ty>)? {
            $(
                $(
                    $(#[$($doc_tt:tt)*])*
                    $field_name:ident: $delegate_type:ty,
                )+
            )?
        })*) => {$(
            macro_rules! ctx_type {
                ($$alt_ty:ty) => {
                    $crate::expand_field!(@internal @ty_bind $$alt_ty; $(@alt $ctx_ty)?)
                };
            }

            $crate::struct_packet_components!(@internal
                $(#[$($tt)*])*
                $(
                ///
                /// Packet Field Explanation
                /// ---
                #[doc="<table><thead><tr><th>Field</th><th>Description</th></tr></thead><tbody>"]
                $(
                #[doc=concat!(
                    "<tr><td>",
                    stringify!($field_name),
                    "</td><td>"
                )]
                #[doc=$crate::expand_field!(@internal @doc $(#[$($doc_tt)*])*)]
                $(#[$($doc_tt)*])*
                #[doc="</td></tr>"]
                )+
                #[doc="</tbody></table>"]
                )?
                $(
                    @expand {ctx_type!(())} $(
                        $field_name: $delegate_type,
                    )+
                )?
                @ $struct_name
            );

            $crate::expand_field!(@internal @impl_bind $struct_name, C $(@alt $ctx_ty)? {
                type ComponentType = Self;

                fn decode<'a, A: $crate::prelude::AsyncRead + Unpin + ?Sized>(
                    __context: &'a mut ctx_type!(C),
                    __read: &'a mut A,
                ) -> std::pin::Pin<Box<dyn std::future::Future<Output = $crate::transport::Result<Self>> + 'a>>
                where
                    Self: Sized,
                {
                    Box::pin(async move {
                        $($(
                            $crate::expand_field!(@internal @de_bind __context: ctx_type!(C), __read, $field_name, $delegate_type);
                        )+)?
                        Ok(Self $({
                            $(
                                $field_name,
                            )+
                        })?)
                    })
                }

                fn encode <'a, A: $crate::prelude::AsyncWrite + Unpin + ?Sized> (
                    component_ref: &'a Self,
                    __context: &'a mut ctx_type!(C),
                    __write: & 'a mut A,
                ) -> std::pin::Pin<Box<dyn std::future::Future<Output = $crate::transport::Result<()>> + 'a>> {
                    Box::pin(async move {
                        $($(
                        {
                            let __temp = &component_ref.$field_name;
                            $crate::expand_field!(@internal @ser_bind __context: ctx_type!(C), __write, __temp, $delegate_type);
                        }
                        )+)?
                        Ok(())
                    })
                }

                fn size(component_ref: &Self, __context: &mut ctx_type!(C)) -> $crate::transport::Result<$crate::transport::packet::Size> {
                    let constant_counter = 0;
                    let dynamic_counter = 0;

                    $(
                    let mut constant_counter = constant_counter;
                    let mut dynamic_counter = dynamic_counter;
                    $({
                        let __temp = & component_ref.$field_name;
                        $crate::expand_field!(@internal @size_bind __context: ctx_type!(C), constant_counter, dynamic_counter, __temp, $delegate_type);
                    })+
                    )?

                    if constant_counter == dynamic_counter {
                        Ok($crate::transport::packet::Size::Constant(constant_counter))
                    } else {
                        Ok($crate::transport::packet::Size::Dynamic(dynamic_counter))
                    }
                }
            });
        )*};
    }
}

#[cfg(feature = "tcp-shield")]
mod tcp_shield {
    use std::future::Future;
    use std::pin::Pin;

    use tokio::io::{AsyncRead, AsyncWrite};

    use crate::prelude::{DraxReadExt, DraxWriteExt, PacketComponent};
    use crate::transport::packet::Size;

    pub struct TcpShieldHeaderDelegate;

    impl<C> PacketComponent<C> for TcpShieldHeaderDelegate {
        type ComponentType = String;

        fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
            context: &'a mut C,
            read: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>
        {
            Box::pin(async move {
                let _ = read.read_var_int().await?;
                let out = String::decode(context, read).await?;
                let _ = u16::decode(context, read).await?;
                let _ = read.read_var_int().await?;
                Ok(out)
            })
        }

        fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
            component_ref: &'a Self::ComponentType,
            context: &'a mut C,
            write: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
            Box::pin(async move {
                write.write_var_int(0).await?;
                String::encode(component_ref, context, write).await?;
                u16::encode(&0, context, write).await?;
                write.write_var_int(0x02).await?;
                Ok(())
            })
        }

        fn size(input: &Self::ComponentType, context: &mut C) -> Size {
            match input.size_owned(context) {
                Size::Dynamic(x) => Size::Dynamic(x + 4),
                Size::Constant(x) => Size::Constant(x + 4),
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use crate::prelude::{PacketComponent, Size};
    use crate::transport::packet::primitive::VarInt;

    crate::struct_packet_components! {
        #[derive(Debug, Eq, PartialEq)]
        Example<String> {
            v_int: VarInt,
            uu: i32,
        }
        #[derive(Debug, Eq, PartialEq)]
        _Example2 {
            v_int: VarInt,
            uu: i32,
        }
    }

    crate::enum_packet_components! {
        #[derive(Debug, Eq, PartialEq)]
        ExampleEnum {
            key: VarInt,
            Variant1 {
                v_int: VarInt,
                reg_int: i32,
            },
            Variant2 {
                reg_int: i32,
                v_int: VarInt,
            }
        }
    }

    #[tokio::test]
    async fn test_decode_packet() -> crate::prelude::Result<()> {
        let mut v = vec![25, 0, 0, 0, 10];
        let mut cursor = Cursor::new(&mut v);
        let example = Example::decode(&mut format!(""), &mut cursor).await?;
        let expected = Example {
            v_int: 25i32,
            uu: 10i32,
        };
        assert_eq!(example.v_int, 25);
        assert_eq!(example.uu, 10);
        assert_eq!(example, expected);
        Ok(())
    }

    #[tokio::test]
    async fn test_encode_packet() -> crate::prelude::Result<()> {
        let mut cursor = Cursor::new(vec![0; 5]);
        let example = Example {
            v_int: 25i32,
            uu: 10i32,
        };
        Example::encode(&example, &mut format!(""), &mut cursor).await?;
        assert_eq!(cursor.into_inner(), vec![25, 0, 0, 0, 10]);
        Ok(())
    }

    #[tokio::test]
    async fn test_size_packet() -> crate::prelude::Result<()> {
        let example = Example {
            v_int: 25i32,
            uu: 10i32,
        };
        assert_eq!(Example::size(&example, &mut format!(""))?, Size::Dynamic(5));
        Ok(())
    }

    #[tokio::test]
    async fn test_decode_enum_packet() -> crate::prelude::Result<()> {
        let mut v = vec![0, 25, 0, 0, 0, 10];
        let mut cursor = Cursor::new(&mut v);
        let example = ExampleEnum::decode(&mut (), &mut cursor).await?;
        let expected = ExampleEnum::Variant1 {
            v_int: 25,
            reg_int: 10,
        };
        assert_eq!(example, expected);

        let mut v = vec![1, 0, 0, 0, 10, 25];
        let mut cursor = Cursor::new(&mut v);
        let example = ExampleEnum::decode(&mut (), &mut cursor).await?;
        let expected = ExampleEnum::Variant2 {
            reg_int: 10,
            v_int: 25,
        };
        assert_eq!(example, expected);
        Ok(())
    }

    #[tokio::test]
    async fn test_encode_enum_packet() -> crate::prelude::Result<()> {
        let mut cursor = Cursor::new(vec![0; 6]);
        let example = ExampleEnum::Variant1 {
            v_int: 25,
            reg_int: 10,
        };
        ExampleEnum::encode(&example, &mut (), &mut cursor).await?;
        assert_eq!(cursor.into_inner(), vec![0, 25, 0, 0, 0, 10]);

        let mut cursor = Cursor::new(vec![0; 6]);
        let example = ExampleEnum::Variant2 {
            reg_int: 10,
            v_int: 25,
        };
        ExampleEnum::encode(&example, &mut (), &mut cursor).await?;
        assert_eq!(cursor.into_inner(), vec![1, 0, 0, 0, 10, 25]);
        Ok(())
    }

    #[tokio::test]
    async fn test_size_enum_packet() -> crate::prelude::Result<()> {
        let example = ExampleEnum::Variant1 {
            v_int: 25,
            reg_int: 10,
        };
        assert_eq!(ExampleEnum::size(&example, &mut ())?, Size::Dynamic(6));

        let example = ExampleEnum::Variant2 {
            reg_int: 10,
            v_int: 25,
        };
        assert_eq!(ExampleEnum::size(&example, &mut ())?, Size::Dynamic(6));
        Ok(())
    }
}
