
use candle_core::{DType, Device, Tensor, D};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::{api::sync::Api, Repo, RepoType};
use tokenizers::{PaddingParams, Tokenizer};
use rayon::prelude::*; // For parallel processing

// Improved embedder with attention masks and proper pooling
pub struct LocalEmbedder {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
    max_length: usize,
}

impl LocalEmbedder {
    pub fn new(model_id: &str, quantized: bool) -> Result<Self> {
        let device = Device::cuda_if_available(0)?;
        let api = Api::new()?;
        let repo = api.repo(Repo::new(model_id.to_string(), RepoType::Model));

        // Load config
        let config: Config = serde_json::from_str(&std::fs::read_to_string(
            repo.get("config.json")?,
        )?)?;

        // Load tokenizer with padding
        let mut tokenizer = Tokenizer::from_file(repo.get("tokenizer.json")?).map_err(anyhow::Error::msg)?;
        tokenizer.with_padding(Some(PaddingParams {
            pad_to_multiple_of: 8,
            ..Default::default()
        }));

        // Model weights (quantized or full precision)
        let weights_file = if quantized {
            repo.get("model-Q4_0.gguf")? // Pre-download quantized version
        } else {
            repo.get("model.safetensors")?
        };

        let vb = VarBuilder::from_safetensors(vec![weights_file], DType::F32, &device)?;
        let model = BertModel::load(vb, &config)?;

        Ok(Self {
            model,
            tokenizer,
            device,
            max_length: 256, // Optimize for common snippet lengths
        })
    }

    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // Process in parallel chunks
        texts.par_chunks(8) // Optimal batch size for CPU/GPU
            .map(|batch| self.process_batch(batch))
            .collect()
    }

    fn process_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // Tokenize with attention masks
        let tokens = self.tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(anyhow::Error::msg)?;

        // Prepare inputs
        let token_ids: Vec<Tensor> = tokens.iter()
            .map(|t| Tensor::new(t.get_ids(), &self.device))
            .collect::<Result<_, _>>()?;
        
        let attention_mask: Vec<Tensor> = tokens.iter()
            .map(|t| Tensor::new(t.get_attention_mask(), &self.device))
            .collect::<Result<_, _>>()?;

        let token_ids = Tensor::stack(&token_ids, 0)?;
        let attention_mask = Tensor::stack(&attention_mask, 0)?.to_dtype(DType::F32)?;

        // Forward pass with attention masks
        let outputs = self.model.forward(&token_ids, &attention_mask)?;

        // Proper mean pooling with attention masks
        let weights = attention_mask.broadcast_as(outputs.shape())?;
        let sum_embeddings = (&outputs * &weights)?.sum_keepdim(1)?;
        let sum_weights = weights.sum_keepdim(1)?.clamp(1e-9, f32::MAX)?;
        let embeddings = (sum_embeddings / sum_weights)?;

        // Normalize
        let embeddings = embeddings.broadcast_div(&embeddings.sqr()?.sum_keepdim(1)?.sqrt()?)?;

        // Convert to Vec<Vec<f32>>
        let mut results = Vec::with_capacity(texts.len());
        for i in 0..texts.len() {
            let row = embeddings.i((i, ..))?;
            results.push(row.to_vec1()?);
        }

        Ok(results)
    }

    // Quantization helper
    pub fn quantize_model(model_id: &str) -> Result<()> {
        // Requires llama.cpp or other quant tools
        // Implementation would use ggml for quantization
        unimplemented!("See https://github.com/llamafox/llama_cpp for quantization workflow")
    }
}
