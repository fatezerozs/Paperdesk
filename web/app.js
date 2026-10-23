'use strict';
const $ = (s, root = document) => root.querySelector(s);
const $$ = (s, root = document) => [...root.querySelectorAll(s)];
const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const labels = {pending:'待审核', valuable:'有价值', discarded:'已舍弃', queued:'等待领取',running:'执行中',review:'待审核结果',accepted:'已采纳',rejected:'已取消'};
const state = {view:'library',papers:[],questions:[],quizzes:[],jobs:[],paper:null,selected:[],editing:null,dirty:false,quiz:null,answers:false,source:false};
let toastTimer, searchTimer, loadSeq=0;
function hasOriginal(q){return state.papers.some(p=>p.id===q.paper_id&&["pdf","txt"].includes(p.kind));}
function toast(message,error=false){const t=$('#toast');t.textContent=message;t.className=error?'error':'';t.hidden=false;clearTimeout(toastTimer);toastTimer=setTimeout(()=>t.hidden=true,error?8000:3500);}
async function api(path, options={}){
  const headers={'X-Paperdesk':'1',...options.headers};
  if(options.body && !(options.body instanceof FormData)){headers['Content-Type']='application/json';options.body=JSON.stringify(options.body);}
  const response=await fetch('/api'+path,{...options,headers});
  let data;try{data=await response.json();}catch{throw new Error(`请求失败（${response.status}），请检查服务是否运行。`);}
  if(!response.ok)throw new Error(data.error||`请求失败（${response.status}）`);
  return data;
}
function run(fn){return async event=>{try{await fn(event);}catch(e){toast(e.message,true);}};}
function filter(){const d=$('#difficulty-filter').value;return Object.fromEntries(Object.entries({q:$('#search').value.trim(),status:$('#status').value,subject:$('#subject-filter').value.trim(),tag:$('#tag-filter').value.trim(),paper_id:state.paper,min_difficulty:d,max_difficulty:d}).filter(([,v])=>v!==null&&v!==''));}
function empty(title,text,action=''){return `<div class="empty"><div class="empty-symbol">▤</div><h2>${esc(title)}</h2><p>${esc(text)}</p>${action}</div>`;}
function badge(status){return `<span class="badge ${esc(status)}">${esc(labels[status]||status)}</span>`;}
function fmt(date){return new Date(date).toLocaleDateString('zh-CN');}
function paperName(id){return state.papers.find(p=>p.id===id)?.name||'手工题目 / Agent 变式';}
function sourceLink(q){return q.paper_id?`/api/papers/${encodeURIComponent(q.paper_id)}/source#page=${q.page_start}`:'';}
function mayLeave(){return !state.dirty || confirm('当前题目有未保存的修改，是否放弃修改？');}
function show(view){
  state.view=view;$$('main>section').forEach(s=>s.hidden=s.id!==`${view}-view`);
  $$('.nav').forEach(n=>n.classList.toggle('active',n.dataset.view===view));
  $('#breadcrumb').textContent={library:'题库 / '+(state.paper?paperName(state.paper):'全部题目'),quizzes:'测验集 / 随机组卷',agents:'Agent 工作台 / 任务与审核',editor:'题库 / 编辑题目'}[view];
  window.scrollTo({top:0});
}
async function refreshMeta(){
  [state.papers,state.quizzes,state.jobs]=await Promise.all([api('/papers'),api('/quizzes'),api('/jobs')]);
  $('#quiz-count').textContent=state.quizzes.length;$('#job-count').textContent=state.jobs.filter(j=>['queued','running','review'].includes(j.status)).length;
  $('#papers').innerHTML=state.papers.length?state.papers.map(p=>`<div class="paper-item"><button class="paper-link ${p.id===state.paper?'active':''}" data-paper="${esc(p.id)}" title="${esc(p.name)}">${esc(p.name)}</button><button class="source-edit" data-manage-source="${esc(p.id)}" title="重命名 ${esc(p.name)}" aria-label="重命名 ${esc(p.name)}">⋯</button></div>`).join(''):'<p class="hint">导入的试卷会显示在这里</p>';
}
async function loadLibrary(){
  const seq=++loadSeq;const questions=await api('/questions?'+new URLSearchParams(filter()));if(seq!==loadSeq)return;
  state.questions=questions;state.selected=[];$('#selection-count').textContent='';$('#merge').disabled=true;
  $('#library-count').textContent=questions.length;$('#result-count').textContent=`${questions.length} 道题目 · ${questions.filter(q=>q.status==='pending').length} 道待审核`;
  $('#library-title').textContent=state.paper?paperName(state.paper):'让好题，留下来。';
  if(state.view==='library')$('#breadcrumb').textContent='题库 / '+(state.paper?paperName(state.paper):'全部题目');
  const p=state.papers.find(p=>p.id===state.paper);const notice=$('#paper-notice');notice.hidden=!p;
  if(p)notice.innerHTML=`${esc(p.warning)} ${p.kind!=="manual"?`<a target="_blank" rel="noopener" href="/api/papers/${esc(p.id)}/source">查看原卷 ↗</a>`:""} ${p.kind==="pdf"?'<button id="recognize-paper">重新识别 PDF</button>':""} <button id="open-ocr">导入分页文字</button> <button id="open-structured">导入结构化结果</button> <button data-manage-source="${esc(p.id)}">重命名来源</button>`;
  $('#questions').innerHTML=questions.length?questions.map(q=>`<article class="question-row"><input type="checkbox" aria-label="选择第 ${esc(q.number||'未编号')} 题" data-select="${esc(q.id)}"><div class="question-main" role="button" tabindex="0" data-edit="${esc(q.id)}"><div class="question-meta"><span class="question-no">${esc(q.number?'Q '+q.number:'未编号')}</span>${badge(q.status)}<span>${esc(q.subject)} · 难度 ${q.difficulty}</span><span>${esc(paperName(q.paper_id))}${q.paper_id?' · p.'+q.page_start+(q.page_end!==q.page_start?'–'+q.page_end:''):''}</span></div><div class="question-title">${esc(q.content)}</div><div class="tags">${q.tags.map(t=>`<span>${esc(t)}</span>`).join('')}</div></div><button class="row-end" data-edit="${esc(q.id)}" aria-label="编辑题目">↗</button></article>`).join(''):empty('这里，留给下一道好题。',Object.keys(filter()).length?'没有符合筛选条件的题目，请调整筛选或导入试卷。':'导入 PDF 或文字试卷，拆题后逐一审阅，也可以手工建题。','<button class="button" data-action="import">＋ 导入第一份试卷</button>');
}
async function library(){if(!mayLeave())return;state.dirty=false;show('library');await refreshMeta();await loadLibrary();}
function newQuestion(){return {id:'',paper_id:state.paper,number:'',content:'',page_start:1,page_end:1,subject:'未分类',difficulty:3,tags:[],status:'pending',verified:false,answer:'',explanation:'',parent_id:null,revision:0};}
async function edit(id){if(!mayLeave())return;state.editing=id?await api('/questions/'+id):newQuestion();state.dirty=false;state.source=false;renderEditor();}
function renderEditor(){
  const q=state.editing;state.source=false;show('editor');
  $('#editor-view').innerHTML=`<div class="editor-toolbar"><button class="back" id="editor-back">← 返回题库</button><div><span id="save-state" class="hint">${q.id?'已保存 · 修订 '+q.revision:'新题目'}</span>${hasOriginal(q)?'<button id="toggle-source" class="button">对照原卷</button>':''}<button id="save" class="button primary">保存修改</button></div></div><div class="editor-layout"><article class="document"><p class="eyebrow">${q.parent_id?'AGENT VARIANT · 待人工审核':'QUESTION NOTE'}</p><h1>${q.id?'题目笔记':'写下一道新题'}</h1><div class="question-meta">${esc(paperName(q.paper_id))}${q.parent_id?` · 原题 <button data-edit="${esc(q.parent_id)}">查看 ↗</button>`:''}</div><form id="editor-form"><div class="doc-meta"><label>题号<input name="number" value="${esc(q.number)}" placeholder="例如：12"></label><label>起始页<input name="page_start" type="number" min="1" value="${q.page_start}" required></label><label>结束页<input name="page_end" type="number" min="1" value="${q.page_end}" required></label></div><div class="field-heading">题干<button type="button" id="preview-toggle">预览 Markdown</button></div><textarea name="content" class="question-body" placeholder="在这里写下题目。支持 Markdown 源码与 LaTeX 公式源码。" required>${esc(q.content)}</textarea><div id="content-preview" class="prose-preview" hidden></div><p class="hint">公式保留 LaTeX 源码；原卷中的图片、表格与复杂公式请在右侧核对。</p><div class="doc-meta"><label>学科<input name="subject" value="${esc(q.subject)}" placeholder="例如：数学" required></label><label>难度<select name="difficulty">${[1,2,3,4,5].map(d=>`<option value="${d}" ${q.difficulty===d?'selected':''}>${d} · ${['基础','较易','中等','较难','挑战'][d-1]}</option>`).join('')}</select></label><label>标签<input name="tags" value="${esc(q.tags.join(', '))}" placeholder="用逗号分隔"></label></div><div class="field-heading">答案<span>ANSWER</span></div><textarea name="answer" rows="2" placeholder="填写答案，或交给 agent 生成草稿">${esc(q.answer)}</textarea><div class="field-heading">解析<span>EXPLANATION</span></div><textarea name="explanation" rows="6" placeholder="记录解题步骤、思路与易错点">${esc(q.explanation)}</textarea><label class="checkline"><input type="checkbox" name="verified" ${q.verified?'checked':''}>${hasOriginal(q)?'我已对照原卷核验题干、公式、图表和单位':'我已人工核验题干、公式、图表和单位'}</label><div class="doc-status"><label>题目价值<select name="status">${['pending','valuable','discarded'].map(s=>`<option value="${s}" ${q.status===s?'selected':''}>${labels[s]}</option>`).join('')}</select></label><div>${badge(q.status)}</div></div></form><div class="job-actions"><button id="editor-job" class="button" ${!q.id?'disabled':''}>✧ 撰写解析 / 变式</button><button id="editor-split" class="button" ${!q.id?'disabled':''}>拆分题目</button><button id="editor-delete" class="button danger" ${!q.id?'disabled':''}>删除</button></div></article>${hasOriginal(q)?`<aside class="source-pane" hidden><div><span>原卷 · p.${q.page_start}–${q.page_end}</span><a href="${sourceLink(q)}" target="_blank" rel="noopener">在新窗口打开 ↗</a></div><iframe title="原始试卷预览" src="${sourceLink(q)}"></iframe></aside>`:''}</div>`;
  installSourcePicker(q);
  $('#editor-form').addEventListener('input',()=>{state.dirty=true;$('#save-state').textContent='未保存';});
}
function editorValue(){const f=new FormData($('#editor-form'));return {...state.editing,paper_id:f.get('paper_id')||null,number:f.get('number'),content:f.get('content'),subject:f.get('subject'),difficulty:Number(f.get('difficulty')),page_start:Number(f.get('page_start')),page_end:Number(f.get('page_end')),tags:[...new Set(f.get('tags').split(/[,，]/).map(t=>t.trim()).filter(Boolean))],answer:f.get('answer'),explanation:f.get('explanation'),verified:f.has('verified'),status:f.get('status')};}
async function save(){if(!$('#editor-form').reportValidity())return;const q=editorValue();const sourceChanged=q.paper_id!==state.editing.paper_id;if(!q.id&&(q.status!=='pending'||q.verified)){q.status='pending';q.verified=false;toast('新题先保存为待审核，核验后可标记价值');}state.editing=await api('/questions'+(q.id?'/'+q.id:''),{method:q.id?'PUT':'POST',body:q});state.dirty=false;renderEditor();await refreshMeta();toast(sourceChanged?'来源已更换，题目需重新核验':'题目已保存');}
function markdown(text){return esc(text).replace(/^### (.+)$/gm,'<h3>$1</h3>').replace(/^## (.+)$/gm,'<h2>$1</h2>').replace(/^# (.+)$/gm,'<h2>$1</h2>').replace(/^&gt; (.+)$/gm,'<blockquote>$1</blockquote>').replace(/\*\*([^*\n]+)\*\*/g,'<strong>$1</strong>').replace(/`([^`\n]+)`/g,'<code>$1</code>');}
async function renderQuizzes(){show('quizzes');state.quizzes=await api('/quizzes');$('#quiz-count').textContent=state.quizzes.length;
  $('#quizzes-view').innerHTML=`<div class="page-heading"><div><p class="eyebrow">PRACTICE COLLECTION</p><h1>一次小小的检验。</h1><p class="muted">随机抽题，留一段安静思考的时间。</p></div><button class="button primary" id="quiz-new">＋ 新建测验</button></div>${state.quizzes.length?state.quizzes.map(q=>`<div class="quiz-row"><div><button class="quiz-title" data-quiz="${esc(q.id)}">${esc(q.title)}</button><p class="quiz-info">${q.questions.length} 道题 · ${fmt(q.created_at)} · 独立快照</p></div><div><button class="button" data-quiz="${esc(q.id)}">开始测验 →</button><button class="back" data-delete-quiz="${esc(q.id)}" aria-label="删除测验">×</button></div></div>`).join(''):empty('还没有测验','先在题库筛选学科、标签或难度，再抽取已核验的好题。')}`;
}
async function openQuiz(id){state.quiz=await api('/quizzes/'+id);state.answers=false;renderQuiz();}
function renderQuiz(){show('quizzes');const q=state.quiz;$('#quizzes-view').innerHTML=`<button class="back" id="quizzes-back">← 返回测验集</button><div class="quiz-heading"><p class="eyebrow">PRACTICE / ${q.questions.length} QUESTIONS</p><h1>${esc(q.title)}</h1><p class="quiz-info">${fmt(q.created_at)} · 题目快照已保存 · 作答自动保存在当前浏览器</p><div class="quiz-actions"><button id="show-answers" class="button">${state.answers?'隐藏答案':'查看答案与解析'}</button><button id="quiz-pdf" class="button">导出 PDF</button><button id="quiz-print" class="button">打印当前页面</button><button id="quiz-download" class="button">导出 Markdown</button></div></div>${q.questions.map((x,i)=>`<article class="quiz-question"><div class="question-meta"><span class="question-no">${String(i+1).padStart(2,'0')}</span><span>${esc(x.subject)} · 难度 ${x.difficulty}</span></div><div class="prose">${esc(x.content)}</div>${hasOriginal(x)?`<a class="hint" href="${sourceLink(x)}" target="_blank" rel="noopener">查看原卷图片与公式 ↗</a>`:''}<textarea data-response="${i}" aria-label="第 ${i+1} 题作答" placeholder="写下你的思路与答案…">${esc(readResponse(q.id,i))}</textarea><button class="back" data-job-question="${esc(x.id)}" data-kind="variant">✧ 基于此题创建变式任务</button>${state.answers?`<div class="quiz-answer"><strong>答案</strong>\n${esc(x.answer||'暂无答案')}\n\n<strong>解析</strong>\n${esc(x.explanation||'暂无解析，可回题库创建 agent 任务。')}</div>`:''}</article>`).join('')}`;}
function readResponse(id,index){try{return localStorage.getItem(`paperdesk:${id}:${index}`)||'';}catch{return '';}}
async function renderAgents(){show('agents');state.jobs=await api('/jobs');$('#agents-view').innerHTML=`<div class="page-heading"><div><p class="eyebrow">AGENT WORKSPACE</p><h1>把思路，再向前一步。</h1><p class="muted">接入你选择的 agent，撰写解析或生成变式。所有结果先经人工审核。</p></div><button class="button" id="refresh-jobs">刷新任务</button></div><details><summary>如何接入 agent</summary><p class="muted">接口说明见项目 docs/API.md；令牌保存在 data/agent-token.txt。未接入 worker 时，任务保持等待领取。</p><pre class="code">GET  /api/agent/v1/jobs\nPOST /api/agent/v1/jobs/{id}/claim\nPOST /api/agent/v1/jobs/{id}/result\nAuthorization: Bearer &lt;token&gt;\nX-Paperdesk: 1</pre><p class="hint">扫描件可通过 /api/papers/{id}/text 接入外部 OCR。原始 PDF 可通过带认证的 agent source 接口读取。</p></details><div class="jobs-list">${state.jobs.length?state.jobs.map(j=>`<article class="job-card"><header><span>${j.kind==='variant'?'生成变式':'撰写解析'} · ${esc(j.source.number||'未编号')} ${badge(j.status)}</span><span class="hint">${fmt(j.created_at)}</span></header><p>${esc(j.source.content.slice(0,220))}</p>${j.instruction?`<p class="job-instruction">要求：${esc(j.instruction)}</p>`:''}${j.result?`<pre>${j.result.content?'题干\n'+esc(j.result.content)+'\n\n':''}答案\n${esc(j.result.answer)}\n\n解析\n${esc(j.result.explanation)}</pre>`:''}<div class="job-actions">${j.status==='review'?`<button class="button primary" data-accept-job="${esc(j.id)}">${j.kind==='variant'?'采纳为待审核新题':'采纳答案与解析'}</button>`:''}${['queued','running','review'].includes(j.status)?`<button class="button" data-reject-job="${esc(j.id)}">${j.status==='review'?'舍弃结果':'取消任务'}</button>`:''}<button class="back" data-edit="${esc(j.question_id)}">查看原题 →</button></div></article>`).join(''):empty('还没有交给 agent 的任务','打开一道题，点击「撰写解析 / 变式」开始。')}</div>`;}
function jobDialog(id,kind='explanation'){if(state.dirty){toast('请先保存题目再创建任务',true);return;}const f=$('#job-form');f.elements.question_id.value=id;f.elements.kind.value=kind;f.elements.instruction.value='';$('#job-dialog').showModal();}
function quizDialog(){openAdvancedQuiz().catch(e=>toast(e.message,true));}

function download(name,body,type='application/json'){const url=URL.createObjectURL(new Blob([body],{type}));const a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),5000);}
function safeFilename(name){return name.replace(/[<>:"/\\|?*\x00-\x1f]/g,'_').slice(0,100)||'测验';}
document.addEventListener('click',run(async e=>{
 const b=e.target.closest('button,[data-edit]');if(!b)return;
 if(b.dataset.close){$('#'+b.dataset.close).close();return;}
 if(b.dataset.view){if(!mayLeave())return;state.dirty=false;if(b.dataset.view==='library')await library();else if(b.dataset.view==='quizzes')await renderQuizzes();else await renderAgents();return;}
 if(b.dataset.paper){if(!mayLeave())return;state.dirty=false;state.paper=b.dataset.paper;await library();return;}
 if(b.dataset.edit){await edit(b.dataset.edit);return;}
 if(b.dataset.quiz){await openQuiz(b.dataset.quiz);return;}
 if(b.dataset.jobQuestion){jobDialog(b.dataset.jobQuestion,b.dataset.kind);return;}
 if(b.dataset.acceptJob){if(!confirm('确认已检查生成结果并采纳？变式会作为新题重新审核。'))return;await api('/jobs/'+b.dataset.acceptJob+'/accept',{method:'POST'});toast('已采纳');await refreshMeta();await renderAgents();return;}
 if(b.dataset.rejectJob){await api('/jobs/'+b.dataset.rejectJob+'/reject',{method:'POST'});await refreshMeta();await renderAgents();return;}
 if(b.dataset.deleteQuiz){if(!confirm('删除这份测验？题库中的题目会保留。'))return;await api('/quizzes/'+b.dataset.deleteQuiz,{method:'DELETE'});await renderQuizzes();return;}
 if(b.dataset.action==='import'){$('#file').click();return;}
 switch(b.id){
 case 'import':if(mayLeave())$('#file').click();break;
 case 'all-papers':if(!mayLeave())return;state.dirty=false;state.paper=null;await library();break;
 case 'new-question':await edit(null);break;
 case 'make-quiz':case 'quiz-new':quizDialog();break;
 case 'save':await save();break;
 case 'editor-back':await library();break;
 case 'toggle-source':state.source=!state.source;$('.source-pane').hidden=!state.source;$('.editor-layout').classList.toggle('with-source',state.source);break;
 case 'preview-toggle':{const t=$('[name=content]',$('#editor-form'));const preview=$('#content-preview');preview.hidden=!preview.hidden;t.hidden=!preview.hidden;preview.innerHTML=markdown(t.value);b.textContent=preview.hidden?'预览 Markdown':'继续编辑';break;}
 case 'editor-job':jobDialog(state.editing.id);break;
 case 'editor-delete':if(!confirm('永久删除这道题？已保存的测验快照会保留。'))return;await api('/questions/'+state.editing.id,{method:'DELETE'});state.dirty=false;toast('题目已删除');await library();break;
 case 'editor-split':if(state.dirty){toast('请先保存修改',true);return;}$('#split-form').elements.parts.value=state.editing.content;$('#split-dialog').showModal();break;
 case 'merge':if(!confirm('按勾选顺序合并题目？合并后需重新审核，原答案和解析会清空。'))return;await api('/questions/merge',{method:'POST',body:{ids:state.selected}});toast('已合并，请重新核验');await library();break;
 case 'recognize-paper':
   if(!confirm('重新识别只替换未编辑的待审候选；人工编辑或审核过的题目会受到保护。继续？'))return;
   b.disabled=true;b.textContent='正在本地识别…';
   try{const result=await api('/papers/'+state.paper+'/recognize',{method:'POST'});await library();toast(`识别完成，生成 ${result.count} 道待审候选`);}finally{b.disabled=false;b.textContent='重新识别 PDF';}break;
  case 'open-ocr':$('#ocr-form').reset();$('#ocr-dialog').showModal();break;
 case 'export':download('paperdesk-export.json',JSON.stringify(await api('/export'),null,2));toast('已导出结构化数据；原卷请另行备份 data/papers');break;
 case 'refresh-jobs':await refreshMeta();await renderAgents();break;
 case 'quizzes-back':await renderQuizzes();break;
 case 'show-answers':state.answers=!state.answers;renderQuiz();break;
 case 'quiz-pdf':await openPdfDialog();break;
 case 'quiz-print':window.print();break;
 case 'quiz-download':{const q=state.quiz;download(safeFilename(q.title)+'.md',`# ${q.title}\n\n`+q.questions.map((x,i)=>`## ${i+1}.\n\n${x.content}\n\n${state.answers?`### 答案\n${x.answer}\n\n### 解析\n${x.explanation}\n\n`:''}`).join(''),'text/markdown;charset=utf-8');break;}
 }
}));
document.addEventListener('keydown',run(async e=>{if((e.ctrlKey||e.metaKey)&&e.key==='s'&&state.view==='editor'){e.preventDefault();await save();}if((e.key==='Enter'||e.key===' ')&&e.target.matches('.question-main')){e.preventDefault();await edit(e.target.dataset.edit);}}));
document.addEventListener('change',e=>{if(e.target.dataset.select){const id=e.target.dataset.select;state.selected=e.target.checked?[...state.selected,id]:state.selected.filter(x=>x!==id);$('#selection-count').textContent=state.selected.length?`已选 ${state.selected.length}`:'';$('#merge').disabled=state.selected.length<2;}});
document.addEventListener('input',e=>{if(e.target.dataset.response!==undefined){try{localStorage.setItem(`paperdesk:${state.quiz.id}:${e.target.dataset.response}`,e.target.value);}catch{toast('浏览器存储不可用，作答无法自动保存',true);}}});
['search','subject-filter','tag-filter'].forEach(id=>$('#'+id).addEventListener('input',()=>{clearTimeout(searchTimer);searchTimer=setTimeout(run(loadLibrary),250);}));
['status','difficulty-filter'].forEach(id=>$('#'+id).addEventListener('change',run(loadLibrary)));
$('#file').addEventListener('change',run(async e=>{const file=e.target.files[0];if(!file)return;if(file.size>32*1024*1024)throw new Error('文件不能超过 32 MB');const form=new FormData();form.append('file',file);const btn=$('#import');btn.disabled=true;btn.textContent='正在识别与拆分…';try{const result=await api('/import',{method:'POST',body:form});state.paper=result.paper.id;state.dirty=false;await library();toast(`已导入，生成 ${result.count} 道候选题，请对照原卷审核。`);}finally{btn.disabled=false;btn.textContent='＋ 导入试卷';e.target.value='';}}));
$('#quiz-form').addEventListener('submit',run(async e=>{e.preventDefault();const result=await api('/quizzes',{method:'POST',body:quizPayload()});$('#quiz-dialog').close();await refreshMeta();await openQuiz(result.id);toast('测验已生成');}));
$('#job-form').addEventListener('submit',run(async e=>{e.preventDefault();await api('/jobs',{method:'POST',body:Object.fromEntries(new FormData(e.target))});$('#job-dialog').close();await refreshMeta();toast('任务已创建，等待外部 agent 领取');}));
$('#split-form').addEventListener('submit',run(async e=>{e.preventDefault();const parts=new FormData(e.target).get('parts').split(/^\s*---\s*$/m).map(t=>t.trim()).filter(Boolean);await api('/questions/'+state.editing.id+'/split',{method:'POST',body:{parts,revision:state.editing.revision}});$('#split-dialog').close();state.dirty=false;await library();toast('题目已拆分，请重新审核');}));
$('#ocr-form').addEventListener('submit',run(async e=>{e.preventDefault();const pages=new FormData(e.target).get('pages').split(/^\s*---\s*$/m);const result=await api('/papers/'+state.paper+'/text',{method:'POST',body:{pages}});$('#ocr-dialog').close();await library();toast(`已生成 ${result.count} 道候选题`);}));
window.addEventListener('beforeunload',e=>{if(state.dirty){e.preventDefault();e.returnValue='';}});
$('#pdf-form').addEventListener('submit',run(async e=>{
  e.preventDefault();const values=new FormData(e.target);const button=$('button[type="submit"]',e.target);
  button.disabled=true;button.textContent='正在生成…';
  try {
    const settings=Object.fromEntries(values);settings.answers=String(values.has('answers'));const query=new URLSearchParams(settings);try{localStorage.setItem('paperdesk:pdf-options',JSON.stringify(settings));}catch{}
    const response=await fetch(`/api/quizzes/${encodeURIComponent(state.quiz.id)}/pdf?${query}`);
    if(!response.ok){const data=await response.json();throw new Error(data.error||'PDF 生成失败');}
    const bytes=await response.arrayBuffer();
    download(safeFilename(state.quiz.title)+(values.has('answers')?'（含解析）':'（试题）')+'.pdf',bytes,'application/pdf');
    $('#pdf-dialog').close();toast('PDF 已导出');
  } finally {button.disabled=false;button.textContent='下载 PDF';}
}));
library().catch(e=>toast(e.message,true));
