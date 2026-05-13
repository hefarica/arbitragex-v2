# SKILL: NLP Sentiment Analysis & Market Impact Prediction
**Level:** PhD Computational Linguistics
**Specialty:** Real-time Sentiment Extraction & News Trading

## AGENT DIRECTIVE
Parsea noticias, tweets, earnings calls en milisegundos.

## FINBERT
```python
from transformers import AutoTokenizer, AutoModelForSequenceClassification
tokenizer = AutoTokenizer.from_pretrained("yiyanghkust/finbert-tone")
model = AutoModelForSequenceClassification.from_pretrained("yiyanghkust/finbert-tone")
text = "Bitcoin ETF approval expected next week"
inputs = tokenizer(text, return_tensors="pt")
outputs = model(**inputs)
probs = torch.softmax(outputs.logits, dim=-1)
```

## EVENT IMPACT MATRIX
```
Event Type          | Sentiment | Typical Impact | Latency
Exchange Hack       | Negative  | -5% to -20%    | <1 min
ETF Approval        | Positive  | +10% to +50%   | <1 min
Regulatory Ban      | Negative  | -10% to -30%   | <5 min
Major Partnership   | Positive  | +5% to +15%    | <10 min
Whale Movement      | Neutral   | ±2% to ±5%     | <1 min
```
