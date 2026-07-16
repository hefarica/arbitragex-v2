'use client';

import { useState } from 'react';

interface TranslationResponse {
  source_word: string;
  translation_type: string;
  mathematical_expression: string;
  semantic_explanation: string;
}

export default function TranslatorPage() {
  const [input, setInput] = useState('');
  const [result, setResult] = useState<TranslationResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const handleTranslate = async () => {
    if (!input.trim()) return;
    setLoading(true);
    setError('');
    try {
      const res = await fetch(`/api/translate?word=${encodeURIComponent(input)}`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      setResult(data);
    } catch (err: any) {
      setError(err.message || 'Translation failed');
      setResult(null);
    } finally {
      setLoading(false);
    }
  };

  const formulas = [
    { label: 'SVD', desc: 'Singular Value Decomposition (Extraction)' },
    { label: 'RFD', desc: 'Radial Flow Divergence (Drainage)' },
    { label: 'HBA', desc: 'Hysteresis Loop Analysis (Manipulation)' },
    { label: 'IES', desc: 'Entropy Inflation (Pump)' },
    { label: 'DCL', desc: 'Limit Discontinuity (Rug Pull)' },
  ];

  return (
    <div className="min-h-screen bg-zinc-950 text-white p-8">
      <div className="max-w-4xl mx-auto">
        <h1 className="text-4xl font-bold mb-2 bg-gradient-to-r from-cyan-400 to-purple-500 bg-clip-text text-transparent">
          Fact-Forcing Gate
        </h1>
        <p className="text-zinc-400 mb-8">
          Semantic Translation Engine — Financial terms to pure mathematics
        </p>

        <div className="bg-zinc-900 rounded-xl p-6 mb-8 border border-zinc-800">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="Enter financial term (e.g., 'sacar', 'drenar', 'manipular')..."
            className="w-full h-32 bg-zinc-800 rounded-lg p-4 text-white placeholder-zinc-500 resize-none focus:outline-none focus:ring-2 focus:ring-cyan-500"
          />
          <button
            onClick={handleTranslate}
            disabled={loading}
            className="mt-4 px-8 py-3 bg-gradient-to-r from-cyan-500 to-purple-600 rounded-lg font-semibold hover:opacity-90 disabled:opacity-50 transition"
          >
            {loading ? 'Computing...' : 'Translate to Math'}
          </button>
        </div>

        {error && (
          <div className="bg-red-900/30 border border-red-800 rounded-xl p-4 mb-8 text-red-300">
            {error}
          </div>
        )}

        {result && (
          <div className="bg-zinc-900 rounded-xl p-6 border border-zinc-800 space-y-4">
            <div className="flex items-center gap-4">
              <span className="text-zinc-500">Source:</span>
              <span className="text-xl font-mono text-cyan-400">{result.source_word}</span>
            </div>
            <div className="flex items-center gap-4">
              <span className="text-zinc-500">Type:</span>
              <span className="px-3 py-1 bg-purple-500/20 text-purple-300 rounded-full text-sm font-mono">
                {result.translation_type}
              </span>
            </div>
            <div>
              <span className="text-zinc-500 block mb-2">Expression:</span>
              <code className="block bg-zinc-950 rounded-lg p-4 font-mono text-green-400 text-lg">
                {result.mathematical_expression}
              </code>
            </div>
            <div>
              <span className="text-zinc-500 block mb-2">Explanation:</span>
              <p className="text-zinc-300">{result.semantic_explanation}</p>
            </div>
          </div>
        )}

        <div className="mt-12 grid grid-cols-5 gap-4">
          {formulas.map((f) => (
            <div key={f.label} className="bg-zinc-900 rounded-lg p-4 border border-zinc-800 text-center">
              <div className="text-2xl font-bold text-cyan-400 mb-1">{f.label}</div>
              <div className="text-xs text-zinc-500">{f.desc}</div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
