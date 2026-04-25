// the_lab/prompt.rs — Multi-format prompt builder
// Supports ChatML, Llama2, Llama3, Gemma, Phi, Mistral, and more.

use super::gguf::PromptFormat;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::System => write!(f, "system"),
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

pub struct PromptBuilder;

impl PromptBuilder {
    /// Build a prompt string for the given format.
    pub fn build(messages: &[ChatMessage], format: PromptFormat) -> String {
        match format {
            PromptFormat::ChatML => Self::build_chat_ml(messages),
            PromptFormat::Llama3 => Self::build_llama3(messages),
            PromptFormat::Llama2 => Self::build_llama2(messages),
            PromptFormat::Gemma => Self::build_gemma(messages),
            PromptFormat::Phi => Self::build_phi(messages),
            PromptFormat::Mistral => Self::build_mistral(messages),
            PromptFormat::DeepSeek => Self::build_chat_ml(messages), // uses ChatML
            PromptFormat::Vicuna => Self::build_vicuna(messages),
            PromptFormat::Alpaca => Self::build_alpaca(messages),
            PromptFormat::Raw => Self::build_raw(messages),
        }
    }

    /// ChatML format (Qwen, Yi, OpenHermes, etc.)
    pub fn build_chat_ml(messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();
        for msg in messages {
            prompt.push_str("<|im_start|>");
            prompt.push_str(&msg.role.to_string());
            prompt.push('\n');
            prompt.push_str(&msg.content);
            prompt.push_str("<|im_end|>\n");
        }
        prompt.push_str("<|im_start|>assistant\n");
        prompt
    }

    /// Llama 3+ format
    fn build_llama3(messages: &[ChatMessage]) -> String {
        let mut prompt = String::from("<|begin_of_text|>");
        for msg in messages {
            prompt.push_str("<|start_header_id|>");
            prompt.push_str(&msg.role.to_string());
            prompt.push_str("<|end_header_id|>\n\n");
            prompt.push_str(&msg.content);
            prompt.push_str("<|eot_id|>");
        }
        prompt.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");
        prompt
    }

    /// Llama 2 format
    fn build_llama2(messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();
        let mut system_text = String::new();

        for msg in messages {
            match msg.role {
                Role::System => {
                    system_text = msg.content.clone();
                }
                Role::User => {
                    prompt.push_str("<s>[INST] ");
                    if !system_text.is_empty() {
                        prompt.push_str("<<SYS>>\n");
                        prompt.push_str(&system_text);
                        prompt.push_str("\n<</SYS>>\n\n");
                        system_text.clear();
                    }
                    prompt.push_str(&msg.content);
                    prompt.push_str(" [/INST]");
                }
                Role::Assistant => {
                    prompt.push(' ');
                    prompt.push_str(&msg.content);
                    prompt.push_str(" </s>");
                }
            }
        }
        // If last message was user, add space for assistant to generate
        if matches!(messages.last().map(|m| &m.role), Some(Role::User)) {
            prompt.push(' ');
        }
        prompt
    }

    /// Google Gemma format
    fn build_gemma(messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();
        for msg in messages {
            let role = match msg.role {
                Role::System | Role::User => "user",
                Role::Assistant => "model",
            };
            prompt.push_str("<start_of_turn>");
            prompt.push_str(role);
            prompt.push('\n');
            prompt.push_str(&msg.content);
            prompt.push_str("<end_of_turn>\n");
        }
        prompt.push_str("<start_of_turn>model\n");
        prompt
    }

    /// Microsoft Phi format
    fn build_phi(messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();
        for msg in messages {
            prompt.push_str("<|");
            prompt.push_str(&msg.role.to_string());
            prompt.push_str("|>\n");
            prompt.push_str(&msg.content);
            prompt.push_str("<|end|>\n");
        }
        prompt.push_str("<|assistant|>\n");
        prompt
    }

    /// Mistral Instruct format
    fn build_mistral(messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();
        for msg in messages {
            match msg.role {
                Role::System | Role::User => {
                    prompt.push_str("[INST] ");
                    prompt.push_str(&msg.content);
                    prompt.push_str(" [/INST]");
                }
                Role::Assistant => {
                    prompt.push_str(&msg.content);
                    prompt.push_str("</s>");
                }
            }
        }
        prompt
    }

    /// Vicuna format
    fn build_vicuna(messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();
        for msg in messages {
            let role = match msg.role {
                Role::System => "SYSTEM",
                Role::User => "USER",
                Role::Assistant => "ASSISTANT",
            };
            prompt.push_str(role);
            prompt.push_str(": ");
            prompt.push_str(&msg.content);
            prompt.push('\n');
        }
        prompt.push_str("ASSISTANT: ");
        prompt
    }

    /// Alpaca format
    fn build_alpaca(messages: &[ChatMessage]) -> String {
        let mut prompt = String::new();
        for msg in messages {
            match msg.role {
                Role::System => {
                    prompt.push_str("### System:\n");
                    prompt.push_str(&msg.content);
                    prompt.push_str("\n\n");
                }
                Role::User => {
                    prompt.push_str("### Instruction:\n");
                    prompt.push_str(&msg.content);
                    prompt.push_str("\n\n");
                }
                Role::Assistant => {
                    prompt.push_str("### Response:\n");
                    prompt.push_str(&msg.content);
                    prompt.push_str("\n\n");
                }
            }
        }
        prompt.push_str("### Response:\n");
        prompt
    }

    /// Raw format (just concatenate)
    fn build_raw(messages: &[ChatMessage]) -> String {
        messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Build the system prompt for Heisenberg.
    pub fn system_prompt() -> String {
        r#"You are Heisenberg — a precise, disciplined local AI agent.
You operate exclusively on the local system with no external network access.
You possess advanced modular tools (Arms). Use the following EXACT tags to execute them:

1. Bash execution:
   <execute>ls -la</execute>

2. Write a file (creates or overwrites):
   <write_file path="/path/to/file">
   file content here
   </write_file>

3. Edit/append to a file:
   <edit_file path="/path/to/file" mode="append">
   content to append
   </edit_file>
   Modes: "append", "prepend", "replace"
   For replace mode, include search and replace:
   <edit_file path="/path/to/file" mode="replace" search="old text">new text</edit_file>

4. Delete a file:
   <delete_file path="/path/to/file" />

5. Browser navigation:
   <browse>https://example.com</browse>

6. Take a screenshot:
   <capture_screen></capture_screen>

7. Self-modification (diff patch):
   <patch>patch content</patch>

RULES:
- Wait for the system to provide the output of your execution before proceeding.
- Be concise, accurate, and structured.
- When generating code or commands, format them clearly.
- Use <write_file> for creating new files instead of echo/cat in bash when possible.
- Use <edit_file> for modifying existing files.
- Never fabricate information. If uncertain, say so explicitly.
- Say my name."#
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_ml_format() {
        let msgs = vec![
            ChatMessage::system("You are helpful."),
            ChatMessage::user("Hello"),
        ];
        let prompt = PromptBuilder::build_chat_ml(&msgs);
        assert!(prompt.contains("<|im_start|>system"));
        assert!(prompt.contains("<|im_start|>user"));
        assert!(prompt.ends_with("<|im_start|>assistant\n"));
    }

    #[test]
    fn test_llama3_format() {
        let msgs = vec![
            ChatMessage::system("You are helpful."),
            ChatMessage::user("Hi"),
        ];
        let prompt = PromptBuilder::build(&msgs, PromptFormat::Llama3);
        assert!(prompt.contains("<|begin_of_text|>"));
        assert!(prompt.contains("<|start_header_id|>system<|end_header_id|>"));
        assert!(prompt.ends_with("<|start_header_id|>assistant<|end_header_id|>\n\n"));
    }

    #[test]
    fn test_multi_format_dispatch() {
        let msgs = vec![ChatMessage::user("Hello")];
        // Should not panic for any format
        for fmt in [
            PromptFormat::ChatML,
            PromptFormat::Llama3,
            PromptFormat::Llama2,
            PromptFormat::Gemma,
            PromptFormat::Phi,
            PromptFormat::Mistral,
            PromptFormat::DeepSeek,
            PromptFormat::Vicuna,
            PromptFormat::Alpaca,
            PromptFormat::Raw,
        ] {
            let prompt = PromptBuilder::build(&msgs, fmt);
            assert!(!prompt.is_empty(), "Empty prompt for {:?}", fmt);
        }
    }
}
