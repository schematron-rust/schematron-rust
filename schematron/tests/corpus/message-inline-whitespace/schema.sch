<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="a">
      <!-- A message is mixed content. The text between two inline elements
           is character data like any other, and survives into the message. -->
      <assert test="false()">A [<name/> <emph>e</emph>]</assert>

      <!-- The same with text that is not whitespace alone, which every
           implementation preserves; here to show the contrast. -->
      <assert test="false()">B [<name/> and <emph>e</emph>]</assert>

      <!-- And with no text between them at all. -->
      <assert test="false()">C [<name/><emph>e</emph>]</assert>
    </rule>
  </pattern>
</schema>
