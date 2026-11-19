<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="lot">
      <report test="normalize-space(libellé) = '12&#x202F;345&#xA0;€'">Montant préservé.</report>
      <report test="count(id(@refs)) = 1 and count(id(string(@refs))) = 1 and id(@refs)/@xml:id = 'c'">One matching ID.</report>
    </rule>
  </pattern>
</schema>
