#[macro_export]
macro_rules! tryffi {
    ($expr:expr) => {
        match $expr {
            Ok(expr) => expr,
            Err(err) => {
                //crate::rustelo_error::ERROR
                //    .lock()
                //    .replace(failure::Error::from(err));
                println!("Expr error when running {:?}", err);
                return crate::rustelo_error::RusteloResult::Failure;
            }
            
        }
    };
}