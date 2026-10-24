/* KaTeX stays local: no document text is sent to a rendering service. */
'use strict';
function mathEscape(value){return String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));}
function mathContent(value){
  const text=String(value??''),parts=[];
  // Protect code and math before applying the small, HTML-escaped Markdown subset.
  const pattern=/```[\s\S]*?```|`[^`\n]+`|\\\[[\s\S]*?\\\]|\\\([\s\S]*?\\\)|\$\$[\s\S]*?\$\$|(?<![\\$])\$(?!\$)(?:\\.|[^$\n])+?(?<!\\)\$(?!\$)/g;
  let last=0,match;
  function prose(s){return mathEscape(s).replace(/^### (.+)$/gm,'<h3>$1</h3>').replace(/^##? (.+)$/gm,'<h2>$1</h2>').replace(/^&gt; (.+)$/gm,'<blockquote>$1</blockquote>').replace(/\*\*([^*\n]+)\*\*/g,'<strong>$1</strong>');}
  while((match=pattern.exec(text))){
    parts.push(prose(text.slice(last,match.index)));const token=match[0];last=pattern.lastIndex;
    if(token.startsWith('`')){const n=token.startsWith('```')?3:1;parts.push(`<code>${mathEscape(token.slice(n,-n))}</code>`);continue;}
    const display=token.startsWith('$$')||token.startsWith('\\['),n=token.startsWith('\\')||display?2:1,formula=token.slice(n,-n);
    try{
      if(formula.length>8000)throw new Error('公式过长，请拆成多个公式');
      if(!window.katex)throw new Error('公式渲染资源未加载');
      parts.push(katex.renderToString(formula,{displayMode:display,throwOnError:true,trust:false,strict:'ignore',maxExpand:1000,maxSize:20,output:'htmlAndMathml'}));
    }catch(error){parts.push(`<span class="math-error" title="${mathEscape(error.message)}">${mathEscape(token)} <small>公式待检查</small></span>`);}
  }
  parts.push(prose(text.slice(last)));return parts.join('');
}
function installMathPreviews(){
  for(const name of ['answer','explanation']){
    const field=document.querySelector(`#editor-form [name=${name}]`),details=document.createElement('details');
    details.className='math-preview-details';details.innerHTML='<summary>预览'+(name==='answer'?'答案':'解析')+'与公式</summary><div class="prose-preview"></div>';
    field.after(details);const update=()=>{details.querySelector('div').innerHTML=mathContent(field.value);};
    details.addEventListener('toggle',()=>{if(details.open)update();});field.addEventListener('input',()=>{if(details.open)update();});
  }
}
async function printMathQuiz(quiz,settings){
  // Open synchronously from the submit gesture, before awaiting fonts or images.
  const page=window.open('about:blank','_blank');if(!page)throw new Error('浏览器拦截了排版窗口，请允许此站点打开弹窗后重试');
  const clamp=(v,min,max,fallback)=>Number.isFinite(Number(v))?Math.min(max,Math.max(min,Number(v))):fallback;
  const fontSize=clamp(settings.font_size,9,16,11.5),spacing=clamp(settings.line_spacing,1.2,2.4,1.65),margin=clamp(settings.margin,12,35,20),lines=clamp(settings.answer_lines,0,12,3),columns=settings.columns==='2'?2:1;
  const size={a4:'A4',a5:'A5',letter:'letter'}[settings.page_size]||'A4',fontId=encodeURIComponent(settings.font_id||'builtin-sans');
  const answers=settings.answers==='true',body=quiz.questions.map((q,i)=>`<article><h2>${i+1}. <span>${mathEscape(q.subject)} · 难度 ${q.difficulty}</span></h2><div class="prose">${mathContent(q.content)}</div><div class="answer-space" style="height:${lines*spacing}em"></div></article>`).join('');
  const answerBody=answers?`<section class="solutions"><h1>参考答案与解析</h1>${quiz.questions.map((q,i)=>`<article><h2>第 ${i+1} 题</h2><div class="prose">${mathContent(q.answer||'暂无答案')}</div><div class="prose">${mathContent(q.explanation||'暂无解析')}</div></article>`).join('')}</section>`:'';
  const origin=location.origin,doc=page.document;
  const printQuery=new URLSearchParams({font_id:settings.font_id||'builtin-sans',font_size:fontSize,line_spacing:spacing,margin,columns,page_size:settings.page_size||'a4'});
  doc.open();doc.write(`<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><title>${mathEscape(quiz.title)}</title><link rel="stylesheet" href="${origin}/vendor/katex/katex.min.css"><link rel="stylesheet" href="${origin}/print.css?${mathEscape(printQuery)}"></head><body><div class="print-toolbar"><button id="print" disabled>正在加载字体…</button><span id="print-hint">排版完成后选择“另存为 PDF”，关闭浏览器页眉页脚。</span></div><header><h1>${mathEscape(quiz.title)}</h1><p>共 ${quiz.questions.length} 题　姓名：____________　日期：____________</p></header><section class="questions">${body}</section>${answerBody}</body></html>`);doc.close();
  try{
    await new Promise((resolve,reject)=>{if(doc.readyState==='complete')resolve();else{page.addEventListener('load',resolve,{once:true});page.setTimeout(()=>reject(new Error('排版资源加载超时，请关闭排版窗口后重试')),20000);}});
    await doc.fonts.load(`${fontSize}px QuizText`);
    await doc.fonts.ready;
    if(!doc.fonts.check(`${fontSize}px QuizText`))throw new Error('所选字体加载失败，请重新选择字体');
    const button=doc.getElementById('print');button.disabled=false;button.textContent='打印 / 保存为 PDF';button.addEventListener('click',()=>page.print());
    const errors=doc.querySelectorAll('.math-error').length;if(errors)doc.getElementById('print-hint').textContent=`有 ${errors} 处公式无法排版，已保留源码。请先回题库检查。`;
    toast('已打开公式排版窗口，可打印为 PDF');
  }catch(error){if(!page.closed)doc.getElementById('print-hint').textContent=error.message;throw error;}
}
