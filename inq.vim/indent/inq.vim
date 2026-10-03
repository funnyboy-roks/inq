setlocal indentexpr=InqIndent()

function! InqIndent(...)
  let line = getline(v:lnum)
  let previousNum = prevnonblank(v:lnum - 1)
  let previous = getline(previousNum)

  if previous =~ "{" && previous !~ "}" && line !~ "}" && line !~ ":$" || previous =~ "route" && previous !~ ";"
    return indent(previousNum) + &tabstop
  else
    return indent(previousNum)
  endif
endfunction
