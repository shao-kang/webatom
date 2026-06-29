import type { DocumentContext } from '@/interface/document-context';
import type { NodeHandle } from '@/interface/native';
import type { Node } from '@/interface/node';

type NodeFactory = (ctx: DocumentContext, handle: NodeHandle) => Node;

const nodeRegistry = new Map<number, NodeFactory>();

export function registerNodeType(nodeType: number, factory: NodeFactory): void {
  nodeRegistry.set(nodeType, factory);
}

export function getNodeFactory(nodeType: number): NodeFactory | undefined {
  return nodeRegistry.get(nodeType);
}
