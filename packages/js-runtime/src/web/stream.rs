use crate::event_loop::HostBridge;
use crate::extension::Extension;

pub struct StreamExtension;

impl Extension for StreamExtension {
    fn name(&self) -> &'static str {
        "stream"
    }

    fn native_module_specifiers(&self) -> &'static [&'static str] {
        &[]
    }

    fn install(&self, _ctx: &rquickjs::Ctx<'_>, _host: &HostBridge) -> rquickjs::Result<()> {
        Ok(())
    }

    fn js_glue(&self) -> Option<&'static str> {
        Some(JS_GLUE)
    }
}

const JS_GLUE: &str = r#"
class ReadableStream {
    constructor(underlyingSource) {
        this._queue = [];
        this._readers = [];
        this._closed = false;
        this._error = null;
        this.locked = false;

        const controller = {
            enqueue: (chunk) => this._enqueue(chunk),
            close: () => this._close(),
            error: (e) => this._errorStream(e),
        };

        if (underlyingSource && underlyingSource.start) {
            underlyingSource.start(controller);
        }
    }

    _enqueue(chunk) {
        if (this._readers.length > 0) {
            const { resolve } = this._readers.shift();
            resolve({ value: chunk, done: false });
        } else {
            this._queue.push(chunk);
        }
    }

    _close() {
        this._closed = true;
        for (const { resolve } of this._readers) {
            resolve({ value: undefined, done: true });
        }
        this._readers = [];
    }

    _errorStream(e) {
        this._error = e;
        for (const { reject } of this._readers) {
            reject(e);
        }
        this._readers = [];
    }

    getReader() {
        if (this.locked) throw new TypeError('ReadableStream is already locked to a reader');
        this.locked = true;
        return new ReadableStreamDefaultReader(this);
    }

    tee() {
        const branch1 = new ReadableStream();
        const branch2 = new ReadableStream();
        const reader = this.getReader();
        (async () => {
            try {
                while (true) {
                    const { value, done } = await reader.read();
                    if (done) {
                        branch1._close();
                        branch2._close();
                        break;
                    }
                    branch1._enqueue(value);
                    branch2._enqueue(value);
                }
            } catch (e) {
                branch1._errorStream(e);
                branch2._errorStream(e);
            } finally {
                reader.releaseLock();
            }
        })();
        return [branch1, branch2];
    }

    pipeTo(destination, { signal } = {}) {
        const reader = this.getReader();
        const writer = destination.getWriter();
        return (async () => {
            try {
                while (true) {
                    if (signal && signal.aborted) throw new TypeError('Pipe aborted');
                    const { value, done } = await reader.read();
                    if (done) break;
                    await writer.write(value);
                }
                await writer.close();
            } catch (e) {
                await writer.abort(e).catch(() => {});
                throw e;
            } finally {
                reader.releaseLock();
                writer.releaseLock();
            }
        })();
    }

    pipeThrough({ writable, readable }, options) {
        this.pipeTo(writable, options);
        return readable;
    }
}

class ReadableStreamDefaultReader {
    constructor(stream) {
        this._stream = stream;
    }

    read() {
        const stream = this._stream;
        return new Promise((resolve, reject) => {
            if (stream._queue.length > 0) {
                resolve({ value: stream._queue.shift(), done: false });
            } else if (stream._closed) {
                resolve({ value: undefined, done: true });
            } else if (stream._error !== null) {
                reject(stream._error);
            } else {
                stream._readers.push({ resolve, reject });
            }
        });
    }

    releaseLock() {
        this._stream.locked = false;
    }

    cancel() {
        return Promise.resolve();
    }
}

class WritableStream {
    constructor(underlyingSink = {}) {
        this._sink = underlyingSink;
        this._state = 'writable'; // 'writable' | 'erroring' | 'errored' | 'closing' | 'closed'
        this._error = undefined;
        this._writer = null;
        // Chain all writes so they execute sequentially.
        this._pendingWrite = Promise.resolve();

        let closedResolve, closedReject;
        this._closedPromise = new Promise((res, rej) => {
            closedResolve = res;
            closedReject = rej;
        });
        this._closedResolve = closedResolve;
        this._closedReject = closedReject;

        if (underlyingSink.start) {
            try {
                underlyingSink.start({ error: (e) => this._errorSink(e) });
            } catch (e) {
                this._errorSink(e);
            }
        }
    }

    get locked() {
        return this._writer !== null;
    }

    getWriter() {
        if (this.locked) throw new TypeError('WritableStream is already locked to a writer');
        this._writer = new WritableStreamDefaultWriter(this);
        return this._writer;
    }

    abort(reason) {
        if (this.locked) return Promise.reject(new TypeError('WritableStream is locked to a writer'));
        return this._abort(reason);
    }

    _errorSink(e) {
        if (this._state === 'writable' || this._state === 'closing') {
            this._state = 'errored';
            this._error = e;
            this._closedReject(e);
        }
    }

    _abort(reason) {
        this._state = 'errored';
        this._error = reason;
        this._closedReject(reason);
        const p = this._sink.abort ? Promise.resolve(this._sink.abort(reason)) : Promise.resolve();
        return p.catch(() => {});
    }

    _write(chunk) {
        const p = this._pendingWrite.then(() => {
            if (this._state !== 'writable') {
                throw this._error !== undefined ? this._error : new TypeError('WritableStream is not in a writable state');
            }
            return Promise.resolve(this._sink.write ? this._sink.write(chunk) : undefined);
        }).catch((e) => {
            if (this._state === 'writable') this._errorSink(e);
            throw e;
        });
        // Keep the chain alive even if this write rejects.
        this._pendingWrite = p.catch(() => {});
        return p;
    }

    _close() {
        const p = this._pendingWrite.then(() => {
            if (this._state !== 'writable') {
                throw this._error !== undefined ? this._error : new TypeError('WritableStream is not in a writable state');
            }
            this._state = 'closing';
            return Promise.resolve(this._sink.close ? this._sink.close() : undefined);
        }).then(() => {
            this._state = 'closed';
            this._closedResolve();
        }).catch((e) => {
            if (this._state !== 'closed') this._errorSink(e);
            throw e;
        });
        this._pendingWrite = p.catch(() => {});
        return p;
    }
}

class WritableStreamDefaultWriter {
    constructor(stream) {
        this._stream = stream;
    }

    get closed() {
        return this._stream._closedPromise;
    }

    get ready() {
        return Promise.resolve();
    }

    get desiredSize() {
        return this._stream._state === 'writable' ? 1 : 0;
    }

    write(chunk) {
        return this._stream._write(chunk);
    }

    close() {
        return this._stream._close();
    }

    abort(reason) {
        return this._stream._abort(reason);
    }

    releaseLock() {
        if (this._stream._writer === this) {
            this._stream._writer = null;
        }
    }
}

class TransformStreamDefaultController {
    constructor(readableController) {
        this._readableController = readableController;
    }

    enqueue(chunk) {
        this._readableController.enqueue(chunk);
    }

    terminate() {
        this._readableController.close();
    }

    error(reason) {
        this._readableController.error(reason);
    }

    get desiredSize() {
        return 1;
    }
}

class TransformStream {
    constructor(transformer = {}) {
        let readableController;
        this.readable = new ReadableStream({
            start(c) { readableController = c; }
        });

        const transformController = new TransformStreamDefaultController(readableController);

        if (transformer.start) {
            try {
                transformer.start(transformController);
            } catch (e) {
                readableController.error(e);
            }
        }

        this.writable = new WritableStream({
            write(chunk) {
                if (transformer.transform) {
                    return transformer.transform(chunk, transformController);
                }
                // identity transform
                transformController.enqueue(chunk);
            },
            close() {
                const flush = transformer.flush
                    ? Promise.resolve(transformer.flush(transformController))
                    : Promise.resolve();
                return flush.then(() => readableController.close());
            },
            abort(reason) {
                readableController.error(reason);
            },
        });
    }
}

globalThis.ReadableStream = ReadableStream;
globalThis.ReadableStreamDefaultReader = ReadableStreamDefaultReader;
globalThis.WritableStream = WritableStream;
globalThis.WritableStreamDefaultWriter = WritableStreamDefaultWriter;
globalThis.TransformStream = TransformStream;
globalThis.TransformStreamDefaultController = TransformStreamDefaultController;
"#;
