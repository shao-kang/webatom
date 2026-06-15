
// Registry: nodeType number → factory function
type NodeFactory = (ctx: DocumentContext, handle: NodeHandle) => Node;

const nodeRegistry = new Map<number, NodeFactory>();
export function registerNodeType(nodeType: number, factory: NodeFactory): void {
  nodeRegistry.set(nodeType, factory);
}