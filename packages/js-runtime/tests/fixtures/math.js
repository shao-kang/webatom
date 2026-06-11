export function add(a, b) {
    console.log(a, b);
    return a + b;
}

console.log('math.js');
let num = 0
const interval = setInterval(() => {
    num += 1
    console.log(`第${num}次`)
    if (num > 5) {
        console.log('结束')
        clearInterval(interval)
    }
}, 1000)
export const PI = 3.14159;
