

// #[macro_export]
// macro_rules! model {
//     (
//         $(#[$derive_2:meta])*
//         $pub:vis struct $name:ident $(<$a:tt>)? {
//             $($fields:tt)*
//         }
//     ) => {
//         #[derive(Model)]
//         $(#[$derive_2])*
//         $pub struct $name <'env$(, $a)?> {
//             env: &'env Environment,
//             $($fields)*
//         }
//     }
// }
