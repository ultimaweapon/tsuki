import OutputViewer from '@/components/output-viewer';

export class Context {
  public constructor(readonly output: InstanceType<typeof OutputViewer>) {}

  public write(data: Uint8Array) {
    // We need to clone the data since it is a view into WebAssembly memory, which will be reused
    // after we return to Rust.
    this.output.write(data.slice());
  }

  public writeln(data: Uint8Array) {
    // We need to clone the data since it is a view into WebAssembly memory, which will be reused
    // after we return to Rust.
    this.output.writeln(data.slice());
  }
}
