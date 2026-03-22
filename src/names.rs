//! 識別子の newtype 定義
//!
//! コンパイラ内部で使用される識別子を型レベルで区別し、
//! 関数名・変数名・型名・フィールド名・バリアント名の混同をコンパイル時に検出する。

use std::borrow::Borrow;
use std::fmt;

macro_rules! define_name_type {
    ($(#[doc = $doc:expr])* $name:ident) => {
        $(#[doc = $doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(name: impl Into<String>) -> Self {
                Self(name.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_owned())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }
    };
}

define_name_type! {
    /// ユーザー定義関数の名前
    FunctionName
}

define_name_type! {
    /// 変数の名前 (グローバル・ローカル・パラメータ)
    VariableName
}

define_name_type! {
    /// ユーザー定義型の名前 (enum・struct)
    TypeName
}

define_name_type! {
    /// struct フィールドの名前
    FieldName
}

define_name_type! {
    /// enum バリアントの名前
    VariantName
}
