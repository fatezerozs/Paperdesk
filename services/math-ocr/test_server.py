import base64
import io
import unittest
from pathlib import Path
from PIL import Image
from server import recognize, page_layout, recognize_from_hints

class FakeResult:
    markdown = {'markdown_texts': r'求 $\frac{x^2}{2}$。'}
class FakePipeline:
    def predict(self, source, **options):
        assert options['use_formula_recognition'] is True
        with Image.open(source) as image:
            assert image.size == (100, 80)
        self.source = source
        yield FakeResult()

class ContractTests(unittest.TestCase):
    def payload(self):
        stream = io.BytesIO()
        Image.new('RGB', (100, 80), 'white').save(stream, format='PNG')
        return {'fileType': 1, 'useFormulaRecognition': True, 'file': base64.b64encode(stream.getvalue()).decode()}
    def test_crop_formula_and_temporary_cleanup(self):
        pipeline = FakePipeline()
        result = recognize(pipeline, self.payload())
        self.assertEqual(result['result']['layoutParsingResults'][0]['markdown']['text'], FakeResult.markdown['markdown_texts'])
        self.assertFalse(Path(pipeline.source).exists())
    def test_rejects_paths_urls_and_invalid_formula_flag(self):
        for value in ['https://example.com/input.png', 'C:/private/input.png', '../input.png']:
            payload = self.payload(); payload['file'] = value
            with self.assertRaises(ValueError):
                recognize(FakePipeline(), payload)
        payload = self.payload(); payload['useFormulaRecognition'] = 'false'
        with self.assertRaises(ValueError):
            recognize(FakePipeline(), payload)

class LayoutTests(unittest.TestCase):
    def test_page_layout_disables_formula_transcription_and_preserves_boxes(self):
        class Result:
            json = {'res': {'parsing_res_list':[{'block_bbox':[1,2,90,40], 'block_label':'text', 'block_content':'1. test', 'block_order':0}], 'overall_ocr_res':{'rec_boxes':[[1,2,90,20]], 'rec_texts':['1. test'], 'rec_scores':[0.97]}}}
        class Pipeline:
            def predict(self, source, **options):
                assert options['use_formula_recognition'] is False
                yield Result()
        payload=ContractTests().payload()
        result=page_layout(Pipeline(),payload)
        self.assertEqual((result['width'],result['height']),(100,80))
        self.assertEqual(result['lines'][0]['bbox'],[1,2,90,20])
        self.assertEqual(result['blocks'][0]['category'],'text')

class SpeedTests(unittest.TestCase):
    def test_text_mode_really_disables_formula_model(self):
        class TextPipeline:
            def predict(self,source,**options):
                assert options['use_formula_recognition'] is False
                yield FakeResult()
        payload=ContractTests().payload();payload['useFormulaRecognition']=False
        self.assertFalse(recognize(TextPipeline(),payload)['metrics']['formula_enabled'])
    def test_reuse_standalone_formula_without_repeating_layout_or_text_ocr(self):
        class Formula:
            def __call__(self,images):
                assert len(images)==1
                yield {'rec_formula':'x^2'}
        class Core:
            class formula_recognition_pipeline:
                formula_recognition_model=Formula()
        class Pipeline:
            paddlex_pipeline=Core()
        payload={'useFormulaRecognition':True,'layout_hints':{'lines':[{'bbox':[0,0,100,20],'text':'Question one','confidence':.99},{'bbox':[0,30,80,50],'text':'bad OCR','confidence':.99}],'formulas':[{'bbox':[0,30,80,50]}]}}
        result=recognize_from_hints(Pipeline(),Image.new('RGB',(100,80),'white'),payload)
        self.assertEqual(result['result']['layoutParsingResults'][0]['markdown']['text'],'Question one\n$$x^2$$')
        self.assertTrue(result['metrics']['reused_layout'])
    def test_inline_formula_requires_full_pipeline(self):
        payload={'useFormulaRecognition':True,'layout_hints':{'lines':[{'bbox':[0,0,100,20],'text':'Question x = 1','confidence':.99}],'formulas':[{'bbox':[30,0,70,20]}]}}
        self.assertIsNone(recognize_from_hints(None,Image.new('RGB',(100,80),'white'),payload))

if __name__ == '__main__':
    unittest.main()
