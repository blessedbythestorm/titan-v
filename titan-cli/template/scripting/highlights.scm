(comment) @comment @spell

(block_comment) @comment

(string) @string

(number) @number

(keyword) @keyword

(identifier) @variable

(function_definition
  (identifier) @function.call)  ; Using 'function.call' highlight group

(binary_expression
  (_) @operator)
