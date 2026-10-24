'use strict';
const splitTerms=value=>[...new Set(String(value||'').split(/[,，\n]/).map(s=>s.trim()).filter(Boolean))];
let quizSelectionIds=[],quizPreviewTimer,quizPreviewSequence=0;

function sourceOptions(selected){return '<option value="">无来源 / 独立题目</option>'+state.papers.map(p=>`<option value="${esc(p.id)}" ${p.id===selected?'selected':''}>${esc(p.name)}${p.kind==='manual'?'（手工来源）':''}</option>`).join('');}
function installSourcePicker(q){
  $('#editor-form').insertAdjacentHTML('afterbegin',`<div class="source-picker"><label>题目来源<select name="paper_id" aria-label="题目来源">${sourceOptions(q.paper_id)}</select></label><div class="list-summary"><span>更换来源后需重新核验页码与题干。</span><button type="button" data-new-source="1">＋ 添加手工来源</button></div></div>`);
}
function sourceDialog(id=null){
  const p=state.papers.find(p=>p.id===id),form=$('#source-form');form.reset();
  form.elements.id.value=p?.id||'';form.elements.expected_name.value=p?.name||'';form.elements.name.value=p?.name||'';
  $('#source-dialog-title').textContent=p?'管理来源':'添加手工来源';$('#delete-source').hidden=!id;$('#source-dialog').showModal();
}
function parseNumbers(text){
  const out=[];for(const part of splitTerms(text)){
    const range=part.match(/^(\d+)\s*[-–~至]\s*(\d+)$/);
    if(range){const a=Number(range[1]),b=Number(range[2]);if(b<a||b-a>1000)throw new Error('题号范围需从小到大，每段最多 1001 题');for(let n=a;n<=b;n++)out.push(String(n));}
    else out.push(part);
  }if(out.length>5000)throw new Error('自定义题号不能超过 5000 个');return [...new Set(out)];
}
function selectedValues(select){return [...select.selectedOptions].map(o=>o.value);}
function quizPayload(){
  const f=$('#quiz-form'),v=new FormData(f),n=Number(v.get('count'));
  if(!Number.isInteger(n)||n<1||n>200)throw new Error('题目数量需要 1–200');
  if(v.has('selected_only')&&!quizSelectionIds.length)throw new Error('请先在题库勾选题目');
  return {title:v.get('title'),count:n,filter:{q:v.get('keyword').trim()||null,min_difficulty:Number(v.get('min_difficulty')),max_difficulty:Number(v.get('max_difficulty'))},selection:{subjects:selectedValues(f.elements.subjects),paper_ids:selectedValues(f.elements.paper_ids),tags:splitTerms(v.get('tags')),tag_mode:v.get('tag_mode'),exclude_tags:splitTerms(v.get('exclude_tags')),numbers:parseNumbers(v.get('numbers')),question_ids:v.has('selected_only')?quizSelectionIds:[],exclude_question_ids:splitTerms(v.get('exclude_question_ids')),min_page:v.get('min_page')?Number(v.get('min_page')):null,max_page:v.get('max_page')?Number(v.get('max_page')):null,original_only:v.has('original_only'),exclude_quiz_ids:selectedValues(f.elements.exclude_quiz_ids)},quotas:v.has('use_quotas')?[1,2,3,4,5].map(d=>({difficulty:d,count:Number(v.get('quota_'+d))})):[]};
}
async function previewQuiz(){
  const seq=++quizPreviewSequence,button=$('#quiz-form button[type=submit]');button.disabled=true;
  try{
    const result=await api('/quizzes/preview',{method:'POST',body:quizPayload()});if(seq!==quizPreviewSequence)return;
    $('#quiz-preview').innerHTML=`<strong>${result.eligible} 道符合条件的题目</strong><span>${result.by_difficulty.map((n,i)=>`难度 ${i+1}：${n}`).join('　')}</span><br>${esc(result.message)}`;
    button.disabled=!result.can_create;
  }catch(e){if(seq!==quizPreviewSequence)return;$('#quiz-preview').textContent=e.message;}
}
async function openAdvancedQuiz(){
  const form=$('#quiz-form');form.reset();quizSelectionIds=[...state.selected];
  const base=filter(),data=await api('/catalog');
  form.elements.subjects.innerHTML=data.subjects.map(s=>`<option value="${esc(s)}" ${base.subject===s?'selected':''}>${esc(s)}</option>`).join('');
  form.elements.paper_ids.innerHTML=state.papers.map(p=>`<option value="${esc(p.id)}" ${base.paper_id===p.id?'selected':''}>${esc(p.name)}</option>`).join('');
  form.elements.exclude_quiz_ids.innerHTML=state.quizzes.map(q=>`<option value="${esc(q.id)}">${esc(q.title)} · ${fmt(q.created_at)}</option>`).join('');
  form.elements.keyword.value=base.q||'';form.elements.tags.value=base.tag||'';
  form.elements.min_difficulty.value=base.min_difficulty||'1';form.elements.max_difficulty.value=base.max_difficulty||'5';
  form.elements.selected_only.disabled=!quizSelectionIds.length;$('#quiz-selected-count').textContent=`（${quizSelectionIds.length} 道）`;
  $('#knowledge-tags').innerHTML=data.tags.map(t=>`<option value="${esc(t)}"></option>`).join('');
  $('#quiz-dialog').showModal();await previewQuiz();
}
async function openPdfDialog(){
  const form=$('#pdf-form'),fonts=await api('/fonts');
  form.elements.font_id.innerHTML=fonts.map(f=>`<option value="${esc(f.id)}">${esc(f.name)}</option>`).join('');
  try{const saved=JSON.parse(localStorage.getItem('paperdesk:pdf-options')||'{}');for(const [key,value]of Object.entries(saved)){if(form.elements[key]&&key!=='answers')form.elements[key].value=value;}}catch{}
  if(!form.elements.font_id.value)form.elements.font_id.value='builtin-sans';form.elements.answers.checked=state.answers;$('#pdf-dialog').showModal();
}
document.addEventListener('click',run(async e=>{
  const b=e.target.closest('button');if(!b)return;
  if(b.dataset.manageSource){sourceDialog(b.dataset.manageSource);return;}
  if(b.dataset.newSource||b.id==='new-source'){sourceDialog();return;}
  if(b.id==='upload-font'){$('#font-file').click();return;}
  if(b.id==='open-structured'){$('#structured-form').reset();$('#structured-dialog').showModal();return;}
  if(b.id==='engine-help')$('#engine-dialog').showModal();
}));
$('#quiz-form').addEventListener('input',()=>{clearTimeout(quizPreviewTimer);++quizPreviewSequence;$('#quiz-form button[type=submit]').disabled=true;quizPreviewTimer=setTimeout(previewQuiz,300);});
$('#quiz-form').addEventListener('change',()=>{clearTimeout(quizPreviewTimer);quizPreviewTimer=setTimeout(previewQuiz,100);});
$('#quiz-dialog').addEventListener('close',()=>{clearTimeout(quizPreviewTimer);++quizPreviewSequence;});
$('#source-form').addEventListener('submit',run(async e=>{
  e.preventDefault();const v=new FormData(e.target),id=v.get('id');
  const paper=await api('/papers'+(id?'/'+id:''),{method:id?'PUT':'POST',body:{name:v.get('name'),expected_name:id?v.get('expected_name'):null}});
  $('#source-dialog').close();await refreshMeta();
  if(state.view==='library')await loadLibrary();
  else if(state.view==='editor'){
    const select=$('#editor-form select[name=paper_id]'),current=select.value;select.innerHTML=sourceOptions(current);
    if(!id){select.value=paper.id;state.dirty=true;$('#save-state').textContent='未保存';}
    else if(!state.dirty)renderEditor();
  }
  toast(id?'来源已重命名，题目关联保留':'手工来源已添加');
}));
$('#structured-form').addEventListener('submit',run(async e=>{
  e.preventDefault();const values=new FormData(e.target);let body;try{body=JSON.parse(values.get('document'));}catch{throw new Error('请输入合法 JSON');}
  body.replace_pending=values.has('replace_pending');const result=await api('/papers/'+state.paper+'/structured',{method:'POST',body});
  $('#structured-dialog').close();await library();toast(`已导入 ${result.count} 道结构化候选题`);
}));
$('#font-file').addEventListener('change',run(async e=>{
  const file=e.target.files[0];if(!file)return;const button=$('#upload-font');button.disabled=true;button.textContent='正在上传…';
  try{const body=new FormData();body.append('file',file);const font=await api('/fonts',{method:'POST',body});const option=new Option(font.name,font.id,true,true);$('#pdf-form').elements.font_id.add(option);toast('字体已添加');}
  finally{button.disabled=false;button.textContent='＋ 上传字体';e.target.value='';}
}));
