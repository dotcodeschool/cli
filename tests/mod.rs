#[cfg(test)]
mod test {
    use std::time::Duration;

    #[test]
    fn foo() {
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(0, 0);
    }

    #[test]
    fn bazz() {
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(0, 1);
    }
}
