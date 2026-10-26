(() => {
  const root = document.getElementById('luma-signature');
  if (!root) return false;
  let range = null, picture = null, dirty = false;
  const notice = text => {
    let element=document.getElementById('luma-signature-notice');
    if(!element){element=document.createElement('p');element.id='luma-signature-notice';element.setAttribute('role','status');root.before(element);}
    element.textContent=text;
  };
  const saveRange = () => {
    const s = getSelection();
    if (s.rangeCount && root.contains(s.anchorNode)) range = s.getRangeAt(0).cloneRange();
  };
  document.addEventListener('selectionchange', saveRange);
  root.addEventListener('input', () => { dirty = true; saveRange(); });
  root.addEventListener('paste', e => {
    const data=e.clipboardData;
    if(data?.files.length||[...(data?.items||[])].some(item=>item.kind==='file')) {
      e.preventDefault();notice('Use Insert picture to upload and approve a hosted image copy.');return;
    }
    const text=data?.getData('text/html')||data?.getData('text/plain')||'';
    if(root.innerHTML.length+text.length>65536){e.preventDefault();notice('Signatures are limited to 64 KiB.');}
  });
  root.addEventListener('drop',e=>{e.preventDefault();notice('Use Insert picture for files, or paste a bounded text selection.');});
  root.addEventListener('beforeinput',e=>{if(e.data&&root.innerHTML.length+e.data.length>65536){e.preventDefault();notice('Signatures are limited to 64 KiB.');}});
  root.addEventListener('click', e => {
    picture = e.target.tagName === 'IMG' ? e.target : null;
    if (picture) { const r = document.createRange(); r.selectNode(picture); getSelection().removeAllRanges(); getSelection().addRange(r); saveRange(); }
  });
  for (const image of root.querySelectorAll('img')) {
    image.dataset.lumaSource = image.getAttribute('src') || '';
    image.removeAttribute('src');
  }
  const restore = () => {
    root.focus();
    if (!range || !root.contains(range.commonAncestorContainer)) {
      range = document.createRange(); range.selectNodeContents(root); range.collapse(false);
    }
    const selection = getSelection(); selection.removeAllRanges(); selection.addRange(range);
  };
  const esc = s => s.replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  window.lumaSignature = {
    command(name, value) {
      restore();
      if (name === 'fontSizePt') {
        const size = Number(value); if (!(size >= 8 && size <= 72)) return false;
        document.execCommand('insertHTML', false, `<span data-luma-font-size="1" style="font-size:${size}pt">${getSelection().rangeCount ? (()=>{const d=document.createElement('div');d.append(getSelection().getRangeAt(0).cloneContents());return d.innerHTML || '&#8203;';})() : '&#8203;'}</span>`);
        const span=root.querySelector('[data-luma-font-size]');
        if(span){span.removeAttribute('data-luma-font-size');const r=document.createRange();r.selectNodeContents(span);r.collapse(false);getSelection().removeAllRanges();getSelection().addRange(r);}
      } else if (name === 'insertImage') {
        const image = JSON.parse(value);
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
        document.execCommand('insertHTML', false, '<table style="border-collapse:collapse"><tbody>'+Array.from({length:rows},()=>'<tr>'+Array.from({length:columns},()=>'<td style="border:1px solid #999;padding:6px"><br></td>').join('')+'</tr>').join('')+'</tbody></table><br>');
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
      const copy = root.cloneNode(true);
      for (const image of copy.querySelectorAll('img')) {
        const source = image.dataset.lumaSource;
        if (!source || !/^https:\/\//i.test(source)) throw new Error('Use Insert picture to add a hosted image before saving.');
        image.src = source; image.removeAttribute('data-luma-source');
      }
      const html = copy.innerHTML;
      if (html.length > 65536) throw new Error('The signature exceeds 64 KiB.');
      return html;
    },
    replace(html) { root.innerHTML=html;for(const image of root.querySelectorAll('img')) {image.dataset.lumaSource=image.getAttribute('src')||'';image.removeAttribute('src');}range=null;picture=null;dirty=false; },
    images() { return [...root.querySelectorAll('img')].map(i=>i.dataset.lumaSource).filter(Boolean).slice(0,8); },
    image(url, data) { for (const image of root.querySelectorAll('img')) if (image.dataset.lumaSource === url) image.src = data; },
    dirty() { return dirty; },
    saved() { dirty = false; }
  };
  root.focus(); return true;
})()
