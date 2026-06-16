(function () {
    try {
        const html  = document.createElement('html');
        const head  = document.createElement('head');
        const title = document.createElement('title');
        const body  = document.createElement('body');
        const h1    = document.createElement('h1');
        const p     = document.createElement('p');
        
        

        // document.appendChild(root,  html);
        html.appendChild(head);
        head.appendChild(title);
        title.appendChild(document.createTextNode('Hello webAtom'));
        html.appendChild(body);
        body.appendChild(h1);
        h1.appendChild(document.createTextNode('标题'));
        body.appendChild(p);
        p.appendChild(document.createTextNode('段落内容'));
         

        function serialize(node, depth) {
            const indent = '  '.repeat(depth);
            const tagName   = document.tagName(node);
            const nodeType = node.nodeType
            console.log('tagName', tagName, node.nodeType)
            let line = `${indent} <${tagName} >`;
            if      (nodeType === Node.DOCUMENT_NODE) line = indent + '#document';
            else if (nodeType === Node.ELEMENT_NODE) line = indent + '<' + node.nodeName + '>';
            else if (nodeType === Node.TEXT_NODE) line = `${indent} #text "${node.textContent}"`;
            else if (nodeType === Node.COMMENT_NODE) line = `${indent} <!-- ${node.textContent} -->`;
            let out = line + '\n';
            let child = node.firstChild;
            console.log('child', child)

            while (child !== null) {
                out  += serialize(child, depth + 1);
                child = child.nextSibling;
            }
            console.log('ddd', out)
            return out;
        }
        // return 'ddd'
        return serialize(html, 0);
    } catch (e) {
        return 'ERRORs: ' + (e && e.message ? e.message : String(e));
    }
})()