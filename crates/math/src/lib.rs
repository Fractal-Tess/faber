/// Adds two numbers together
/// 
/// # Arguments
/// 
/// * `a` - The first number
/// * `b` - The second number
/// 
/// # Returns
/// 
/// The sum of `a` and `b`
/// 
/// # Examples
/// 
/// ```
/// use math::add;
/// 
/// let result = add(2, 3);
/// assert_eq!(result, 5);
/// ```
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
        assert_eq!(add(-1, 1), 0);
        assert_eq!(add(0, 0), 0);
    }
} 