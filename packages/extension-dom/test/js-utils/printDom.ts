const serialize = (node: any, depth = 0): string => {
    const indent = '  '.repeat(depth);
    const type = node.nodeType;
    let line: string;

    if (type === 9) {
        line = indent + '#document';
    } else if (type === 1) {
        const tag = node.tagName;
        let children = '';
        let child = node.firstChild;
        while (child !== null) {
            children += serialize(child, depth + 1);
            child = child.nextSibling;
        }
        return indent + '<' + tag + '>\n' + children + indent + '</' + tag + '>\n';
    } else if (type === 3) {
        line = indent + '#text "' + node.nodeValue + '"';
    } else if (type === 8) {
        line = indent + '<!-- ' + node.nodeValue + ' -->';
    } else {
        line = indent + '[node type=' + type + ']';
    }

    let out = line + '\n';
    let child = node.firstChild;
    while (child !== null) {
        out += serialize(child, depth + 1);
        child = child.nextSibling;
    }
    return out;
};

export const printDom = (node: any): void => {
    console.log(serialize(node));
};
