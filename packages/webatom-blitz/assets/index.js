console.log('hello world');
let frameCount = 0;
let lastTime = 0;

const raf = (ts) => {
    frameCount++;
    if (lastTime === 0) lastTime = ts;
    const elapsed = ts - lastTime;
    if (elapsed >= 1000) {
        console.log('fps: ' + (frameCount / (elapsed / 1000)).toFixed(1));
        frameCount = 0;
        lastTime = ts;
    }
    requestAnimationFrame(raf);
};
requestAnimationFrame(raf);
