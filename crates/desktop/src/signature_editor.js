(() => {
  const root = document.getElementById('luma-signature');
  if (!root) return false;
  let range = null, picture = null, dirty = false;
  const cloneForHistory = (node = root) => {
    if(node.nodeType!==Node.ELEMENT_NODE)return node.cloneNode(false);
    const copy=node.tagName==='IMG'?document.createElement('img'):node.cloneNode(false);
    if(node.tagName==='IMG')for(const attr of node.attributes)if(attr.name!=='src')copy.setAttribute(attr.name,attr.value);
    for(const child of node.childNodes)copy.append(cloneForHistory(child));
    return copy;
  };
  const previews = new Map();
  let current = null, undo = [], redo = [], commanding = false;
  const nodePath = node => {const path=[];while(node&&node!==root){const parent=node.parentNode;if(!parent)return null;path.unshift([...parent.childNodes].indexOf(node));node=parent;}return node===root?path:null;};
  const capture = () => {
    const copy=cloneForHistory();for(const image of copy.querySelectorAll('img'))image.removeAttribute('src');
    const selection=getSelection();let caret=null;
    if(selection.rangeCount&&root.contains(selection.anchorNode)&&root.contains(selection.focusNode))caret=[nodePath(selection.anchorNode),selection.anchorOffset,nodePath(selection.focusNode),selection.focusOffset];
    return {html:copy.innerHTML,caret};
  };
  const trimHistory = () => {
    while(undo.length>64||undo.reduce((n,s)=>n+s.html.length*2+128,0)>1048576)undo.shift();
    const collect=()=>{const used=new Set([...root.querySelectorAll('img')].map(i=>i.dataset.lumaSource));for(const state of [...undo,...redo]){const copy=document.createElement('template');copy.innerHTML=state.html;for(const image of copy.content.querySelectorAll('img'))used.add(image.dataset.lumaSource);}for(const key of previews.keys())if(!used.has(key))previews.delete(key);};
    collect();while([...previews.values()].reduce((n,s)=>n+s.length*2,0)>8388608&&undo.length){undo.shift();collect();}
  };
  const checkpoint = before => {const next=capture();if(before&&before.html!==next.html&&undo.at(-1)?.html!==before.html){undo.push(before);redo=[];dirty=true;}current=next;trimHistory();};
  const restoreState = state => {
    root.innerHTML=state.html;for(const image of root.querySelectorAll('img')){const data=previews.get(image.dataset.lumaSource);if(data)image.src=data;}
    const locate=path=>{let node=root;for(const i of path||[])node=node?.childNodes[i];return node;};
    root.focus({preventScroll:true});const selection=getSelection();selection.removeAllRanges();
    if(state.caret){const [a,ao,z,zo]=state.caret;const first=locate(a),last=locate(z);if(first&&last){const limit=n=>n.nodeType===Node.TEXT_NODE?n.length:n.childNodes.length;selection.setBaseAndExtent(first,Math.min(ao,limit(first)),last,Math.min(zo,limit(last)));}}
    range=selection.rangeCount?selection.getRangeAt(0).cloneRange():null;picture=null;dirty=true;current=capture();
  };
  const step = backwards => {const source=backwards?undo:redo,target=backwards?redo:undo;if(!source.length)return false;target.push(capture());restoreState(source.pop());return true;};
  root.addEventListener('keydown',e=>{if((e.ctrlKey||e.metaKey)&&['z','y'].includes(e.key.toLowerCase())){e.preventDefault();step(e.key.toLowerCase()==='z'&&!e.shiftKey);}});
  root.addEventListener('beforeinput',e=>{if(['historyUndo','historyRedo'].includes(e.inputType)){e.preventDefault();step(e.inputType==='historyUndo');}else current=capture();});
  const notice = text => {
    let element=document.getElementById('luma-signature-notice');
    if(!element){element=document.createElement('p');element.id='luma-signature-notice';element.setAttribute('role','status');root.before(element);}
    element.textContent=text;
  };
  const contentLength = () => {
    const copy=cloneForHistory();
    for(const image of copy.querySelectorAll('img')) {image.setAttribute('src',image.dataset.lumaSource||'');image.removeAttribute('data-luma-source');}
    return copy.innerHTML.length;
  };
  const saveRange = () => {
    const s = getSelection();
    if (s.rangeCount && root.contains(s.anchorNode)) range = s.getRangeAt(0).cloneRange();
  };
  document.addEventListener('selectionchange', saveRange);
  root.addEventListener('input', () => { dirty = true; saveRange(); if(!commanding)checkpoint(current); });
  root.addEventListener('paste', e => {
    const data=e.clipboardData;
    if(data?.files.length||[...(data?.items||[])].some(item=>item.kind==='file')) {
      e.preventDefault();notice('Use Insert picture to upload and approve a hosted image copy.');return;
    }
    const text=data?.getData('text/html')||data?.getData('text/plain')||'';
    if(contentLength()+text.length>65536){e.preventDefault();notice('Signatures are limited to 64 KiB.');}
  });
  root.addEventListener('drop',e=>{e.preventDefault();notice('Use Insert picture for files, or paste a bounded text selection.');});
  root.addEventListener('beforeinput',e=>{if(e.data&&contentLength()+e.data.length>65536){e.preventDefault();notice('Signatures are limited to 64 KiB.');}});
  root.addEventListener('click', e => {
    picture = e.target.tagName === 'IMG' ? e.target : null;
    if (picture) { const r = document.createRange(); r.selectNode(picture); getSelection().removeAllRanges(); getSelection().addRange(r); saveRange(); }
  });
  for (const image of root.querySelectorAll('img')) {
    image.dataset.lumaSource = image.getAttribute('src') || '';
    image.removeAttribute('src');
  }
  const restore = () => {
    saveRange();
    const saved = range;
    root.focus({preventScroll:true});
    range = saved;
    if (!range || !root.contains(range.commonAncestorContainer)) {
      range = document.createRange(); range.selectNodeContents(root); range.collapse(false);
    }
    const selection = getSelection(); selection.removeAllRanges(); selection.addRange(range);
  };
  const esc = s => s.replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  window.lumaSignature = {
    command(name,value) {
      if(name==='undo'||name==='redo')return step(name==='undo');
      restore();const before=capture(),wasDirty=dirty;commanding=true;
      try {
        const result=this.edit(name,value);
        if(contentLength()>65536){restoreState(before);dirty=wasDirty;throw new Error('Signatures are limited to 64 KiB.');}
        if(result)checkpoint(before);return result;
      } catch(error){restoreState(before);dirty=wasDirty;throw error;}
      finally{commanding=false;trimHistory();}
    },
    edit(name, value) {
      restore();
      if (name === 'fontSizePt') {
        const size = Number(value); if (!(size >= 8 && size <= 72)) return false;
        const selection=getSelection();const r=selection.getRangeAt(0);
        const span=document.createElement('span');span.style.fontSize=`${size}pt`;span.append(r.extractContents());
        if(!span.childNodes.length)span.append(document.createTextNode('​'));
        r.insertNode(span);const last=span.lastChild;
        if(last.nodeType===Node.TEXT_NODE){r.setStart(last,last.length);r.collapse(true);}else{r.selectNodeContents(span);r.collapse(false);}
        selection.removeAllRanges();selection.addRange(r);
      } else if (name === 'insertImage') {
        const image = JSON.parse(value);
        previews.set(image.url,image.data);
        if (root.querySelectorAll('img').length >= 8) throw new Error('A signature can contain at most eight pictures.');
        document.execCommand('insertHTML', false, `<img data-luma-source="${esc(image.url)}" src="${esc(image.data)}" width="${Number(image.width)||160}" style="max-width:100%;height:auto" alt="${esc(image.alt||'Signature picture')}">`);
      } else if (name === 'imageWidth') {
        const width = Number(value); if (!picture || !(width >= 16 && width <= 600)) return false;
        picture.width = width; picture.removeAttribute('height'); picture.style.height = 'auto';
      } else if (name === 'removeImage') {
        if (!picture) return false; picture.remove(); picture = null;
      } else if (name === 'insertTable') {
        const [rows, columns] = value.split('x').map(Number);
        if (!(rows >= 1 && rows <= 8 && columns >= 1 && columns <= 8)) return false;
        document.execCommand('insertHTML', false, '<table style="border-collapse:collapse;width:100%"><tbody>'+Array.from({length:rows},()=>'<tr>'+Array.from({length:columns},()=>'<td style="border:1px solid #999;padding:6px"><br></td>').join('')+'</tr>').join('')+'</tbody></table><br>');
      } else if (name === 'lineHeight') {
        const valueNumber = Number(value); if (!(valueNumber >= 1 && valueNumber <= 2)) return false;
        const node = getSelection().anchorNode;
        const block = (node.nodeType === 1 ? node : node.parentElement).closest('div,p,li,td');
        if (block && root.contains(block)) block.style.lineHeight = valueNumber;
      } else {
        const allowed = ['bold','italic','underline','strikeThrough','superscript','subscript','insertUnorderedList','insertOrderedList','justifyLeft','justifyCenter','justifyRight','justifyFull','indent','outdent','removeFormat','unlink','undo','redo','fontName','foreColor','hiliteColor','createLink'];
        if (!allowed.includes(name)) return false;
        document.execCommand(name, false, value || null);
      }
      dirty = true; saveRange(); return true;
    },
    snapshot() {
      const copy = cloneForHistory();
      for (const image of copy.querySelectorAll('img')) {
        const source = image.dataset.lumaSource;
        if (!source || !/^https:\/\//i.test(source)) throw new Error('Use Insert picture to add a hosted image before saving.');
        image.src = source; image.removeAttribute('data-luma-source');
      }
      const html = copy.innerHTML;
      if (html.length > 65536) throw new Error('The signature exceeds 64 KiB.');
      return html;
    },
    replace(html) { root.innerHTML=html;for(const image of root.querySelectorAll('img')) {image.dataset.lumaSource=image.getAttribute('src')||'';image.removeAttribute('src');}range=null;picture=null;dirty=false;undo=[];redo=[];current=capture();trimHistory(); },
    images() { return [...root.querySelectorAll('img')].map(i=>i.dataset.lumaSource).filter(Boolean).slice(0,8); },
    image(url, data) {const images=[...root.querySelectorAll('img')].filter(i=>i.dataset.lumaSource===url);if(!images.length)return;previews.set(url,data);for(const image of images)image.src=data;trimHistory();},
    dirty() { return dirty; },
    saved() { dirty = false; }
  };
  root.focus(); current=capture();return true;
})()
