import base64
import io
import unittest
from pathlib import Path
from PIL import Image
from server import recognize

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
    def test_rejects_paths_urls_and_disabled_formulas(self):
        for value in ['https://example.com/input.png', 'C:/private/input.png', '../input.png']:
            payload = self.payload(); payload['file'] = value
            with self.assertRaises(ValueError):
                recognize(FakePipeline(), payload)
        payload = self.payload(); payload['useFormulaRecognition'] = False
        with self.assertRaises(ValueError):
            recognize(FakePipeline(), payload)

if __name__ == '__main__':
    unittest.main()
