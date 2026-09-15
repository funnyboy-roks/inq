" Vim syntax file
" Language: Inq
" Maintainer: funnyboy_roks

syn keyword inqLet let

" http methods
syn keyword inqGet GET
syn keyword inqHead HEAD
syn keyword inqPost POST
syn keyword inqPut PUT
syn keyword inqDelete DELETE
syn keyword inqOptions OPTIONS
syn keyword inqTrace TRACE
syn keyword inqPatch PATCH

syn keyword inqRequest  request
syn keyword inqResponse response

syn keyword inqControlFlow if then else before after

syn match inqBool 'true\|false' display
syn match inqNull 'null' display

syn match inqIdent "\%([^[:cntrl:][:space:][:punct:][:digit:]]\|_\)\%([^[:cntrl:][:punct:][:space:]]\|_\)*" contained
syn match inqInterpolated "$\%([^[:cntrl:][:space:][:punct:][:digit:]]\|_\)\%([^[:cntrl:][:punct:][:space:]]\|_\)*" contains=inqIdent contained

" Inline comments with //
syn keyword inqTodo contained TODO FIXME XXX NOTE
syn match inqComment "//.*$" contains=inqTodo

" Block comments with /* */ (supporting nesting)
syn region inqCommentBlock     matchgroup=inqCommentBlock start="/\*" end="\*/" contains=inqTodo,inqCommentBlockNest,@Spell
syn region inqCommentBlockNest matchgroup=inqCommentBlock start="/\*" end="\*/" contains=inqTodo,inqCommentBlockNest,@Spell contained transparent

" Binary/Octal/Hex integers: 0b/0o/0x, followed by digit, followed by digit or _
syn match inqNumber '[-+]\?0b[01][01_]*'    display
syn match inqNumber '[-+]\?0o\o\%(\o\|_\)*' display
syn match inqNumber '[-+]\?0x\x\%(\x\|_\)*' display

" Keyword number #inf, #-inf, #nan
" syn match inqNumber '#\%(-\?inf\|nan\)' contained display

" Floating point numbers (containing '.' and/or 'E'/'e')
syn match inqNumber '[-+]\?\d[[:digit:]_]*\%(\.\d[[:digit:]_]*\)\?\%([eE][-+]\?\d[[:digit:]_]*\)\?' display

" Try to error for invalid escape sequence (\<invalid character>, \u{not 1-6 chars}, \u{<non hex char>})
syn match  inqEscapeError    '\\u{\%([^}]*[^[:xdigit:]}][^}]*\|[^}]\{7,}\)\?}\|\\x[^"][^"]\?\|\\.' contained display
" Valid escape codes (\<valid escpae char>, \u{<1-6 hex>})
syn match  inqEscape '\\["nrt\\bfs[:space:]\$]\|\\u{\x\{1,6}}\|\\x\x\x' contained display
syn region inqString start='"'           end='"'      skip='\\\\\|\\"'           display contains=inqEscape,inqEscapeError,inqStringInterpolate,inqInterpolated,@Spell

syn region inqBlock start='{' end='}' transparent fold


" syn region inqFuncArgs start='(' end=')' display
syn match inqFuncName "\%([^[:cntrl:][:space:][:punct:][:digit:]]\|_\)\%([^[:cntrl:][:punct:][:space:]]\|_\)*("he=e-1,me=e-1 display
syn match inqRouteNameSep '/' display contained
syn match inqRouteName    "\w\(\%([^[:cntrl:][:space:][:punct:][:digit:]]\|_\)\%([^[:cntrl:][:punct:][:space:]]\|_\|/\)*\)*" display contained contains=inqRouteNameSep
syn keyword inqRoute route nextgroup=inqRouteName skipwhite

syn region inqStringInterpolate matchgroup=inqInterpolated start='${' end='}' display contains=inqNumber,inqString,inqComment,inqControlFlow,inqBlock,inqFuncName

syn region inqAttribute start='#\[' end='\]' display

syn keyword inqBaseUrl BASE_URL

" syn region inqChildren start="{" end="}" contains=inqString,inqJsonString,inqNumber,inqNode,inqBool,inqNull,inqComment,inqCommentBlock

let b:current_syntax = "inq"

hi def link inqRoute           Keyword
hi def link inqControlFlow     Keyword
hi def link inqLet             Keyword
hi def link inqBool            Boolean
hi def link inqNull            Constant
hi def link inqBaseUrl         Constant
hi def link inqInterpolated    Macro
hi def link inqAttribute       PreProc
hi def link inqRouteName       Function
hi def link inqRouteNameSep    Delimiter
hi def link inqFuncName        Function
hi def link inqJson            Function

hi def link inqRequest Identifier
hi def link inqResponse inqRequest

hi def link inqMethod  Type
hi def link inqGet     inqMethod
hi def link inqHead    inqMethod
hi def link inqPost    inqMethod
hi def link inqPut     inqMethod
hi def link inqDelete  inqMethod
hi def link inqOptions inqMethod
hi def link inqTrace   inqMethod
hi def link inqPatch   inqMethod

hi def link inqTodo         Todo
hi def link inqComment      Comment
hi def link inqCommentBlock inqComment
hi def link inqEscape       Special
hi def link inqEscapeError  Error
hi def link inqString       String
hi def link inqNumber       Number
