//! Custom widgets etc. for egui.

mod status_bar;

pub use status_bar::status_bar;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
