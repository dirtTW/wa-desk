// Captures microphone PCM as 960-sample (60 ms) mono frames at the context's
// sample rate. The main thread creates the context at 16 kHz.
class CallMicProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.buffer = new Float32Array(960);
    this.offset = 0;
  }

  process(inputs) {
    const channel = inputs[0] && inputs[0][0];
    if (!channel) return true;
    for (let i = 0; i < channel.length; i += 1) {
      this.buffer[this.offset++] = channel[i];
      if (this.offset === 960) {
        const pcm = new Int16Array(960);
        for (let j = 0; j < 960; j += 1) {
          const sample = Math.max(-1, Math.min(1, this.buffer[j]));
          pcm[j] = sample < 0 ? sample * 0x8000 : sample * 0x7fff;
        }
        this.port.postMessage(pcm.buffer, [pcm.buffer]);
        this.offset = 0;
      }
    }
    return true;
  }
}

registerProcessor("call-mic", CallMicProcessor);
