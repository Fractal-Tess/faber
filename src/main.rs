use math::add;

fn main() {
    let result = add(5, 3);
    println!("5 + 3 = {}", result);
    
    // Test with negative numbers
    let result2 = add(-2, 7);
    println!("-2 + 7 = {}", result2);
}
