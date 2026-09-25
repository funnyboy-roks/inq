use std::rc::Rc;

use inq_lang::{
    StringExt,
    eval::value::{
        Value, ValueRef,
        native::{Array, Int},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FakerValue();

impl Value for FakerValue {
    fn type_name() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "Faker".into()
    }

    fn type_name_of(&self) -> std::borrow::Cow<'static, str> {
        Self::type_name()
    }

    fn to_string(&self, _out: &mut String) {
        unreachable!("This type is never constructed");
    }

    fn snapshot(&self) -> Rc<dyn Value> {
        unreachable!("This type is never constructed");
    }
    fn eq(&self, _other: ValueRef) -> bool {
        unreachable!("This type is never constructed");
    }

    fn debug(&self, _fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unreachable!("This type is never constructed");
    }

    fn register(registry: &mut inq_lang::eval::registry::Registry<Self>)
    where
        Self: Sized,
    {
        macro_rules! impl_fake {
            ($($name:ident => $section:ident :: $kind:ident),*$(,)?) => {
                $(
                registry.register_static_method(stringify!($name), |_, ()| {
                    use fake::Fake;
                    fake::faker::$section::en::$kind().fake::<String>().intern()
                });
                )*
            };
        }

        registry.register_static_method("password", |ctx, (len_min, len_max): (Int, Int)| {
            use fake::Fake;
            if len_min < 0 || len_max <= 0 {
                return Err(ctx.error("length bounds must be greater than 0"));
            }
            if len_max <= len_min {
                return Err(ctx.error("length max must be greater than length min"));
            }
            Ok(
                fake::faker::internet::en::Password(len_min as usize..len_max as usize)
                    .fake::<String>()
                    .intern(),
            )
        });

        registry.register_static_method("lorem", |ctx, length: Int| {
            if length <= 0 {
                return Err(ctx.error_arg(0, "length must be greater than 0"));
            }
            let length = length as usize;
            Ok(lipsum::lipsum(length).intern())
        });

        registry.register_static_method("paragraphs", |ctx, length: Int| {
            if length <= 0 {
                return Err(ctx.error_arg(0, "length must be greater than 0"));
            }

            let lipsum = lipsum::lipsum(50 * length as usize);
            let sentences = lipsum.split(". ").collect::<Vec<_>>();

            Ok(sentences
                .chunks(5)
                // not sure where the dashes come from
                .map(|c| c.join(". ").replace("--", "").trim().intern())
                .collect::<Array>())
        });

        impl_fake! {
            username => internet::Username,
            email => internet::FreeEmail,
            ip => internet::IP,
            ipv4 => internet::IPv4,
            ipv6 => internet::IPv6,

            first_name => name::FirstName,
            last_name => name::LastName,
            full_name => name::Name,
            title => name::Title,
            name_with_title => name::NameWithTitle,

            currency_code => currency::CurrencyCode,
            currency_name => currency::CurrencyName,
            currency_symbol => currency::CurrencySymbol,

            dir_path => filesystem::DirPath,
            file_extension => filesystem::FileExtension,
            file_name => filesystem::FileName,
            file_path => filesystem::FilePath,
            mime_type => filesystem::MimeType,

            version => filesystem::SemverStable,
            version_unstable => filesystem::SemverUnstable,

            job_field => job::Field,
            job_position => job::Position,
            job_seniority => job::Seniority,
            job_title => job::Title,

            phone_number => phone_number::CellNumber,
        }
    }
}

#[cfg(test)]
mod test {
    use inq_lang::eval_expr;

    use crate::script::base_engine;

    /// just test that everything runs without breaking
    #[test]
    fn everything_works() {
        let e = base_engine();
        eval_expr! { e,
            assert(String.is_instance(debug(Faker.lorem(25))));
            assert(Array.is_instance(debug(Faker.paragraphs(25))));

            assert(String.is_instance(debug(Faker.username())));
            assert(String.is_instance(debug(Faker.password(20, 30))));
            assert(String.is_instance(debug(Faker.email())));
            assert(String.is_instance(debug(Faker.ip())));
            assert(String.is_instance(debug(Faker.ipv4())));
            assert(String.is_instance(debug(Faker.ipv6())));

            assert(String.is_instance(debug(Faker.first_name())));
            assert(String.is_instance(debug(Faker.last_name())));
            assert(String.is_instance(debug(Faker.full_name())));
            assert(String.is_instance(debug(Faker.title())));
            assert(String.is_instance(debug(Faker.name_with_title())));

            assert(String.is_instance(debug(Faker.currency_code())));
            assert(String.is_instance(debug(Faker.currency_name())));
            assert(String.is_instance(debug(Faker.currency_symbol())));

            assert(String.is_instance(debug(Faker.dir_path())));
            assert(String.is_instance(debug(Faker.file_extension())));
            assert(String.is_instance(debug(Faker.file_name())));
            assert(String.is_instance(debug(Faker.file_path())));
            assert(String.is_instance(debug(Faker.mime_type())));

            assert(String.is_instance(debug(Faker.version())));
            assert(String.is_instance(debug(Faker.version_unstable())));

            assert(String.is_instance(debug(Faker.job_field())));
            assert(String.is_instance(debug(Faker.job_position())));
            assert(String.is_instance(debug(Faker.job_seniority())));
            assert(String.is_instance(debug(Faker.job_title())));

            assert(String.is_instance(debug(Faker.phone_number())));
        };
    }
}
